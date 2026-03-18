use std::{
    env,
    fs::{self, File},
    io::{Read, Seek, Stdout},
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Error, anyhow, bail};
use clap::Parser;
use log::{debug, error, info};
use rand::{
    distr::{Alphanumeric, SampleString},
    rng,
};
use ratatui::{
    Frame, Terminal,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Direction, Layout, Rect},
    prelude::CrosstermBackend,
    style::{Style, Stylize},
    text::Line,
    widgets::{
        Block, Borders, Gauge, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Wrap,
    },
};
use rhai::{AST, Dynamic, Engine, FuncArgs, Map, Scope};
use simplelog::Config;
use smartstring::alias;
use tui_input::{Input, backend::crossterm::EventHandler};

const MAGIC_NUM: u64 = 0xFEEDCAFEBABEFACE;

const WELCOME_MESSAGE: &str = r#"Welcome to the Setup Wizard

This wizard will guide you through the installation on your computer. The process will only take a few minutes.

Please close any running applications before continuing to ensure the installation completes successfully.

Click "Next" to continue, or "Quit" to exit the setup at any time."#;

#[derive(SchemaWrite, SchemaRead)]
#[repr(C)]
struct PostExecutableHeader {
    magic: u64,
    size: usize,
}

const POST_EXECUTABLE_HEADER_SIZE: usize = size_of::<PostExecutableHeader>();

enum StepResult {
    Next,
    Back,
    Quit,
    Finish,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
enum Step {
    Init,
    Welcome,
    Path,
    Confirm,
    License,
    CopyFiles,
    Finish,
    Quit,
}

fn act_error(
    term: &mut Terminal<CrosstermBackend<Stdout>>,
    _ctx: &mut InstallationContext,
    err: anyhow::Error,
) -> anyhow::Result<StepResult> {
    let mut board = SelectionBoard {
        options: vec!["Quit".into()],
        selection: 0,
    };

    loop {
        term.draw(|f| {
            let title = Paragraph::new("❌ Error")
                .block(Block::new().borders(Borders::BOTTOM).blue())
                .gray();
            f.render_widget(title, Rect::new(1, 1, f.area().width - 2, 2));

            let mut message = format!("Unrecoverable error because:\n");
            for e in err.chain() {
                message.push_str(&format!(" - {e}\n"));
            }
            let paragrah = Paragraph::new(message).wrap(Wrap::default());
            f.render_widget(
                paragrah,
                Rect {
                    x: 1,
                    y: 6,
                    width: f.area().width - 2,
                    height: f.area().height - 3,
                },
            );
            board.draw(f);
        })
        .context("Failed to initiate draw")?;
        match board.handle_event(&event::read().context("Failed to read event")?) {
            Some(0) => return Ok(StepResult::Finish),
            Some(_) | None => {}
        }
    }
}

impl Step {
    fn render_header(section: &str, f: &mut Frame<'_>, product_name: &str) {
        let title = Paragraph::new(format!("🪄 '{product_name}' Install Wizard..."))
            .block(Block::new().borders(Borders::BOTTOM).blue())
            .white()
            .bold();
        f.render_widget(title, Rect::new(1, 1, f.area().width - 2, 2));
        let title = Paragraph::new(section)
            .block(Block::new().borders(Borders::BOTTOM).blue())
            .gray();
        f.render_widget(title, Rect::new(1, 3, f.area().width - 2, 2));
    }

    fn act_welcome(
        &self,
        term: &mut Terminal<CrosstermBackend<Stdout>>,
        ctx: &mut InstallationContext,
    ) -> anyhow::Result<StepResult> {
        let mut board = SelectionBoard {
            options: vec!["Next".into(), "Quit".into()],
            selection: 0,
        };

        let app_name = ctx.get_app_name().context("Cannot get app name")?;
        loop {
            term.draw(|f| {
                Self::render_header("👋 Welcome", f, &app_name);
                let message = WELCOME_MESSAGE;
                let paragrah = Paragraph::new(message).wrap(Wrap::default());
                f.render_widget(
                    paragrah,
                    Rect {
                        x: 1,
                        y: 6,
                        width: f.area().width - 2,
                        height: f.area().height - 3,
                    },
                );
                board.draw(f);
            })
            .context("Failed to initiate draw")?;
            match board.handle_event(&event::read().context("Failed to read event")?) {
                Some(0) => return Ok(StepResult::Next),
                Some(1) => return Ok(StepResult::Quit),
                Some(_) | None => {}
            }
        }
    }

    fn act_quit(
        &self,
        term: &mut Terminal<CrosstermBackend<Stdout>>,
        ctx: &mut InstallationContext,
    ) -> anyhow::Result<StepResult> {
        let mut board = SelectionBoard {
            options: vec!["Yes".into(), "No".into()],
            selection: 1,
        };

        let app_name = ctx.get_app_name().context("Cannot get app name")?;

        loop {
            term.draw(|f| {
                Self::render_header("🚪 Quit confirmation", f, &app_name);
                let message = r#"Are you sure you want to quit?"#;
                let paragrah = Paragraph::new(message).wrap(Wrap::default());
                f.render_widget(
                    paragrah,
                    Rect {
                        x: 1,
                        y: 6,
                        width: f.area().width - 2,
                        height: f.area().height - 3,
                    },
                );
                board.draw(f);
            })
            .context("Failed to initiate draw")?;
            match board.handle_event(&event::read().context("Failed to read event")?) {
                Some(0) => return Ok(StepResult::Finish),
                Some(1) => return Ok(StepResult::Back),
                Some(_) | None => {}
            }
        }
    }

    fn act_confirm(
        &self,
        term: &mut Terminal<CrosstermBackend<Stdout>>,
        ctx: &mut InstallationContext,
    ) -> anyhow::Result<StepResult> {
        let mut board = SelectionBoard {
            options: vec!["No".into(), "Yes".into(), "Quit".into()],
            selection: 0,
        };
        let app_name = ctx.get_app_name().context("Cannot get app name")?;

        loop {
            term.draw(|f| {
                Self::render_header("🚨 Confirm", f, &app_name);
                let message = r#"The next slide is no return point..

Are you sure you want to proceed"#;
                let paragrah = Paragraph::new(message).wrap(Wrap::default());
                f.render_widget(
                    paragrah,
                    Rect {
                        x: 1,
                        y: 6,
                        width: f.area().width - 2,
                        height: f.area().height - 3,
                    },
                );
                board.draw(f);
            })
            .context("Failed to initiate draw")?;
            match board.handle_event(&event::read().context("Failed to read event")?) {
                Some(0) => return Ok(StepResult::Back),
                Some(1) => return Ok(StepResult::Next),
                Some(2) => return Ok(StepResult::Quit),
                Some(_) | None => {}
            }
        }
    }

    fn act_license(
        &self,
        term: &mut Terminal<CrosstermBackend<Stdout>>,
        ctx: &mut InstallationContext,
    ) -> anyhow::Result<StepResult> {
        let mut board = SelectionBoard {
            options: vec!["Back".into(), "Accept".into(), "Quit".into()],
            selection: 0,
        };

        let message = ctx
            .get_config_value("license")
            .context("Cannot get license")?
            .to_string();
        let message = message.as_str();

        let mut state = ScrollbarState::default();
        let mut scroll = 0;
        let mut count = 0;
        let app_name = ctx.get_app_name().context("Cannot get app name")?;

        loop {
            term.draw(|f| {
                let area = Rect {
                    x: 5,
                    y: 5,
                    width: f.area().width - 10,
                    height: f.area().height - 8,
                };
                count = message
                    .lines()
                    .map(|x| x.len() / area.width as usize + 1)
                    .sum();

                state = state
                    .content_length(count)
                    .viewport_content_length((f.area().height - 8).into());
                Self::render_header("📝 Please read the license", f, &app_name);
                let paragrah = Paragraph::new(message)
                    .wrap(Wrap::default())
                    .scroll((scroll, 0))
                    .centered()
                    .block(
                        Block::new()
                            .borders(Borders::ALL)
                            .padding(Padding::new(1, 1, 1, 1))
                            .title_bottom(
                                Line::from(" Navigation: Up/Down/Home/End ")
                                    .right_aligned()
                                    .dark_gray(),
                            ),
                    )
                    .white()
                    .on_black();

                f.render_widget(paragrah, area);
                f.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight),
                    area,
                    &mut state,
                );

                board.draw(f);
            })
            .context("Failed to initiate draw")?;
            let evt = &event::read().context("Failed to read event")?;
            match evt {
                Event::Key(KeyEvent {
                    code: KeyCode::Down,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    if scroll < count as u16 {
                        scroll += 1;
                        state = state.position(scroll as usize);
                    }
                }
                Event::Key(KeyEvent {
                    code: KeyCode::End,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    scroll = count as u16;
                    state = state.position(scroll as usize);
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Home,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    scroll = 0;
                    state = state.position(scroll as usize);
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Up,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    scroll = scroll.saturating_sub(1);
                    state = state.position(scroll as usize);
                }

                _ => {}
            }
            match board.handle_event(evt) {
                Some(0) => return Ok(StepResult::Back),
                Some(1) => return Ok(StepResult::Next),
                Some(2) => return Ok(StepResult::Quit),
                Some(_) | None => {}
            }
        }
    }

    fn act_path(
        &self,
        term: &mut Terminal<CrosstermBackend<Stdout>>,
        ctx: &mut InstallationContext,
    ) -> anyhow::Result<StepResult> {
        let mut board = SelectionBoard {
            options: vec!["Back".into(), "Next".into(), "Edit".into(), "Quit".into()],
            selection: 1,
        };

        let mut in_edit = false;
        let path = ctx
            .get_config_value("path")
            .context("Cannot get path config variable")?
            .to_string();
        let mut input = Input::new(path);
        let mut last_value = String::new();
        let app_name = ctx.get_app_name().context("Cannot get app name")?;

        loop {
            term.draw(|f| {
                let scroll = input.visual_scroll((f.area().width - 2) as usize);

                Self::render_header("👣 Specifty Path...", f, &app_name);
                let title = Paragraph::new("- Installation Path:").gray();
                f.render_widget(title, Rect::new(1, 10, f.area().width - 2, 1));
                let path = Paragraph::new(input.value())
                    .underlined()
                    .scroll((0, scroll as u16));
                let area = Rect::new(1, 12, f.area().width - 2, 1);
                let path = if in_edit {
                    path.light_yellow().on_blue()
                } else {
                    path.blue().on_black()
                };
                f.render_widget(path, area);
                if in_edit {
                    let x = input.visual_cursor().max(scroll) - scroll;
                    f.set_cursor_position((area.x + x as u16, area.y));
                }
                board.draw(f);
            })
            .context("Failed to initiate draw")?;
            let evt = &event::read().context("Failed to read event")?;

            if !in_edit {
                match board.handle_event(evt) {
                    Some(0) => return Ok(StepResult::Back),
                    Some(1) => {
                        return Ok(StepResult::Next);
                    }
                    Some(2) => {
                        last_value = input.value().into();
                        in_edit = true;
                    }
                    Some(3) => return Ok(StepResult::Quit),
                    Some(_) | None => {}
                }
            } else {
                match evt {
                    Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        kind: KeyEventKind::Press,
                        ..
                    }) => {
                        ctx.insert_key_value_config("path".into(), input.value().into())?;
                        in_edit = false;
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Esc,
                        kind: KeyEventKind::Press,
                        ..
                    }) => {
                        input = input.with_value(last_value.clone());
                        in_edit = false;
                    }

                    e => {
                        input.handle_event(e);
                    }
                }
            }
        }
    }

    fn act_installation(
        &self,
        term: &mut Terminal<CrosstermBackend<Stdout>>,
        ctx: &mut InstallationContext,
    ) -> anyhow::Result<StepResult> {
        let steps = ctx
            .call_function::<Dynamic>("installation_actions", (ctx.config.clone(),))
            .context("failed to call installation actions function")?;
        let steps = steps.as_array_ref().map_err(|e| anyhow!("{e}"))?.clone();
        let app_name = ctx.get_app_name().context("Cannot get app name")?;

        let len = steps.len();
        for (idx, step) in steps.into_iter().enumerate() {
            let percent = (idx * 100 / len) as u16;

            term.draw(|f| {
                Self::render_header("📦 Copying files...", f, &app_name);

                let title = Paragraph::new("Progress.").gray();
                f.render_widget(title, Rect::new(1, 10, f.area().width - 2, 1));
                let path = Gauge::default()
                    .percent(percent)
                    .gauge_style(Style::default().blue())
                    .on_white();
                f.render_widget(path, Rect::new(1, 12, f.area().width - 2, 1));
            })
            .context("Failed to initiate draw")?;
            if !ctx.cli.dry_run {
                step.cast::<InstallAction>().act(ctx)?;
            }
        }

        Ok(StepResult::Next)
    }

    fn act_init(
        &self,
        term: &mut Terminal<CrosstermBackend<Stdout>>,
        ctx: &mut InstallationContext,
    ) -> anyhow::Result<StepResult> {
        let application = env::args().next().ok_or(anyhow!("cannot get arg"))?;

        let mut file = File::open(application)?;

        file.seek(std::io::SeekFrom::End(
            -(POST_EXECUTABLE_HEADER_SIZE as i64),
        ))?;

        let mut buf = [0u8; POST_EXECUTABLE_HEADER_SIZE];

        file.read_exact(&mut buf)?;

        let header = wincode::deserialize::<PostExecutableHeader>(&buf)?;

        if header.magic != MAGIC_NUM || header.size == 0 {
            bail!("File footer is invalid, the file may be corrupted")
        }
        let size = header.size;

        let start = (size + POST_EXECUTABLE_HEADER_SIZE) as i64;
        let start = -start;

        file.seek(std::io::SeekFrom::End(start))?;

        let base_dir = ctx.base_dir.clone();

        let mut zip = ZipArchive::new(file)?;

        for idx in 0..zip.len() {
            {
                let mut entry = zip.by_index(idx)?;
                if entry.is_file() {
                    let enclosed_name = entry
                        .enclosed_name()
                        .ok_or(anyhow!("Cannot get Zip entry name"))?;
                    let parent = enclosed_name
                        .parent()
                        .ok_or(anyhow!("Cannot get parent of entry"))?;
                    let mut path = base_dir.clone().join(parent);
                    fs::create_dir_all(&path)?;
                    path.push(
                        enclosed_name
                            .file_name()
                            .ok_or(anyhow!("Cannot get filename of entry"))?,
                    );
                    let mut out_file = File::create(path)?;
                    std::io::copy(&mut entry, &mut out_file)?;
                }
            }
            let percent = (idx * 100 / zip.len()) as u16;
            term.draw(|f| {
                let title = Paragraph::new("🚀 Extracting")
                    .block(Block::new().borders(Borders::BOTTOM).blue())
                    .gray();
                f.render_widget(title, Rect::new(1, 1, f.area().width - 2, 2));

                let title = Paragraph::new("Extracting.").gray();
                f.render_widget(title, Rect::new(1, 3, f.area().width - 2, 1));
                let gauge = Gauge::default()
                    .percent(percent)
                    .gauge_style(Style::default().blue())
                    .on_white();
                f.render_widget(gauge, Rect::new(1, 4, f.area().width - 2, 1));
            })
            .context("Failed to initiate draw")?;
        }

        std::env::set_current_dir(base_dir)?;
        ctx.load_file("main.rhai").map_err(|e| anyhow!("{e}"))?;

        ctx.call_function::<()>("get_metadata", (ctx.config.clone(),))?;

        Ok(StepResult::Next)
    }

    fn act_finish(
        &self,
        term: &mut Terminal<CrosstermBackend<Stdout>>,
        ctx: &mut InstallationContext,
    ) -> anyhow::Result<StepResult> {
        let mut board = SelectionBoard {
            options: vec!["Finish".into()],
            selection: 0,
        };

        let app_name = ctx.get_app_name().context("Cannot get app name")?;

        loop {
            term.draw(|f| {
                Self::render_header("✨ Finish", f, &app_name);
                let message = r#"Installation is finished successfully..."#;
                let paragrah = Paragraph::new(message).wrap(Wrap::default());
                f.render_widget(
                    paragrah,
                    Rect {
                        x: 1,
                        y: 6,
                        width: f.area().width - 2,
                        height: f.area().height - 3,
                    },
                );
                board.draw(f);
            })
            .context("Failed to initiate draw")?;
            match board.handle_event(&event::read().context("Failed to read event")?) {
                Some(0) => return Ok(StepResult::Finish),
                Some(_) | None => {}
            }
        }
    }

    fn act(
        &self,
        term: &mut Terminal<CrosstermBackend<Stdout>>,
        ctx: &mut InstallationContext,
    ) -> anyhow::Result<StepResult> {
        match self {
            Step::Init => self.act_init(term, ctx),
            Step::Welcome => self.act_welcome(term, ctx),
            Step::Path => self.act_path(term, ctx),
            Step::Confirm => self.act_confirm(term, ctx),
            Step::CopyFiles => self.act_installation(term, ctx),
            Step::Finish => self.act_finish(term, ctx),
            Step::License => self.act_license(term, ctx),
            Step::Quit => self.act_quit(term, ctx),
        }
    }
}

struct SelectionBoard {
    options: Vec<String>,
    selection: usize,
}

impl SelectionBoard {
    fn draw(&self, f: &mut Frame) {
        let len = self.options.len();
        let layout = Layout::new(
            Direction::Horizontal,
            vec![Constraint::Ratio(1, len as u32); len],
        )
        //.horizontal_margin(1)
        .split(Rect {
            x: 0,
            y: f.area().height - 2,
            width: f.area().width,
            height: 1,
        });

        f.render_widget(
            Line::from("<- -> : Select - Enter: Accept")
                .dark_gray()
                .on_gray(),
            Rect {
                x: 0,
                y: f.area().height - 1,
                width: f.area().width,
                height: 1,
            },
        );
        for (i, r) in layout.iter().enumerate() {
            if i == self.selection {
                f.render_widget(
                    Paragraph::new(format!("<{}>", self.options[i]))
                        .centered()
                        .light_yellow()
                        .on_red(),
                    *r,
                );
            } else {
                f.render_widget(
                    Paragraph::new(format!("{}", self.options[i]))
                        .centered()
                        .gray()
                        .on_blue(),
                    *r,
                );
            }
        }
    }

    fn handle_event(&mut self, ev: &Event) -> Option<usize> {
        let len = self.options.len();
        match ev {
            Event::Key(KeyEvent {
                code: KeyCode::Left,
                kind: KeyEventKind::Press,
                ..
            }) => {
                if self.selection == 0 {
                    self.selection = len - 1
                } else {
                    self.selection -= 1;
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Right,
                kind: KeyEventKind::Press,
                ..
            }) => {
                if self.selection == len - 1 {
                    self.selection = 0
                } else {
                    self.selection += 1;
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            }) => {
                return Some(self.selection);
            }

            _ => {}
        }
        None
    }
}

use rhai::plugin::*;
use walkdir::WalkDir;
use wincode::{SchemaRead, SchemaWrite};
use zip::ZipArchive;

#[export_module]
#[allow(non_snake_case)]
#[allow(non_upper_case_globals)]
mod StepModule {
    use crate::Step;

    pub const Welcome: Step = Step::Welcome;
    pub const License: Step = Step::License;
    pub const Path: Step = Step::Path;
    pub const CopyFiles: Step = Step::CopyFiles;
    pub const Confirm: Step = Step::Confirm;
    pub const Finish: Step = Step::Finish;

    #[rhai_fn(global, get = "enum_type", pure)]
    pub fn get_type(my_enum: &mut Step) -> String {
        match my_enum {
            Step::Init => "builtin_init".to_string(),
            Step::Welcome => "builtin_welcome".to_string(),
            Step::License => "builtin_license".to_string(),
            Step::Path => "builtin_path".to_string(),
            Step::CopyFiles => "builtin_copyfiles".to_string(),
            Step::Confirm => "builtin_confirm".to_string(),
            Step::Quit | Step::Finish => {
                panic!("this actions has no after")
            }
        }
    }

    #[rhai_fn(global, get = "value", pure)]
    pub fn get_value(_my_enum: &mut Step) -> Dynamic {
        Dynamic::UNIT
    }

    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(my_enum: &mut Step) -> String {
        format!("{my_enum:?}")
    }

    #[rhai_fn(global, name = "==", pure)]
    pub fn eq(my_enum: &mut Step, my_enum2: Step) -> bool {
        my_enum == &my_enum2
    }
    #[rhai_fn(global, name = "!=", pure)]
    pub fn neq(my_enum: &mut Step, my_enum2: Step) -> bool {
        my_enum != &my_enum2
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
enum InstallAction {
    CopyFile(String, String),
    CreateDir(String),
    ExecuteCommand(String, Vec<String>),
}

impl InstallAction {
    fn act_execute(cmd: &str, args: &[String], _: &InstallationContext) -> anyhow::Result<()> {
        info!("InstallAction: Execute '{cmd}' '{args:?}'");
        Command::new(cmd)
            .args(args)
            .spawn()
            .with_context(|| format!("Cannot execute `{cmd}` command"))?;
        Ok(())
    }

    fn act_create_dir(folder: &str, _: &InstallationContext) -> anyhow::Result<()> {
        fs::create_dir_all(folder).context("Cannot create folder")?;
        Ok(())
    }

    fn act_copy_file(src: &str, dst: &str, ctx: &InstallationContext) -> anyhow::Result<()> {
        let dst_path = ctx.get_config_value("path")?.to_string();

        let dst_path = PathBuf::from(dst_path).join(dst);

        info!("InstallAction: Copy '{src}' '{}'", dst_path.display());

        let parent = dst_path.parent().ok_or(anyhow!("Failed to get parent"))?;
        if !parent.exists() {
            fs::create_dir_all(&parent)
                .with_context(|| format!("Cannot create folder {} ", dst_path.display()))?;
        }

        fs::copy(src, &dst_path)
            .with_context(|| format!("Cannot copy file {src} to {} ", dst_path.display()))?;
        Ok(())
    }

    fn act(&self, ctx: &InstallationContext) -> anyhow::Result<()> {
        match self {
            Self::CopyFile(src, dst) => Self::act_copy_file(&src, &dst, ctx),
            Self::CreateDir(dir) => Self::act_create_dir(dir, ctx),
            Self::ExecuteCommand(cmd, args) => Self::act_execute(cmd, args, ctx),
        }
    }
}

fn load_text_from_file(file: String) -> RhaiResult {
    fs::read_to_string(file).map(|x| x.into()).map_err(|e| {
        Box::new(EvalAltResult::ErrorSystem(
            "Cannot read text from file".into(),
            e.into(),
        ))
    })
}

#[export_module]
#[allow(non_snake_case)]
#[allow(non_upper_case_globals)]
mod InstallActionModule {

    use crate::InstallAction;

    pub fn CopyFileTo(src: String, dst: String) -> InstallAction {
        InstallAction::CopyFile(src, dst)
    }

    pub fn CopyFile(src: String) -> InstallAction {
        InstallAction::CopyFile(src.clone(), src)
    }

    pub fn CopyDir(src: String, dst: String) -> Dynamic {
        let mut res = vec![];
        for entry in WalkDir::new(src.clone()) {
            let entry = entry.unwrap();
            if entry.path().is_file() {
                let path = PathBuf::from(dst.clone())
                    .join(entry.path().strip_prefix(src.clone()).unwrap());
                res.push(Dynamic::from(InstallAction::CopyFile(
                    entry.path().display().to_string(),
                    path.to_string_lossy().to_string(),
                )));
            }
        }
        Dynamic::from_array(res)
    }

    pub fn CreateDir(name: String) -> InstallAction {
        InstallAction::CreateDir(name)
    }

    pub fn ExecuteCommand(cmd: String, args: Dynamic) -> InstallAction {
        InstallAction::ExecuteCommand(
            cmd,
            args.as_array_ref()
                .unwrap()
                .iter()
                .map(|x| x.as_immutable_string_ref().unwrap().to_string())
                .collect(),
        )
    }

    #[rhai_fn(global, get = "enum_type", pure)]
    pub fn get_type(my_enum: &mut InstallAction) -> String {
        match my_enum {
            InstallAction::CopyFile(_, _) => "CopyFile".into(),
            InstallAction::CreateDir(_) => "CreateDir".into(),
            InstallAction::ExecuteCommand(_, _) => "ExecuteCommand".into(),
        }
    }

    #[rhai_fn(global, get = "value", pure)]
    pub fn get_value(_my_enum: &mut InstallAction) -> Dynamic {
        Dynamic::UNIT
    }

    #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
    pub fn to_string(my_enum: &mut InstallAction) -> String {
        format!("{my_enum:?}")
    }

    // '==' and '!=' operators
    #[rhai_fn(global, name = "==", pure)]
    pub fn eq(my_enum: &mut InstallAction, my_enum2: InstallAction) -> bool {
        my_enum == &my_enum2
    }
    #[rhai_fn(global, name = "!=", pure)]
    pub fn neq(my_enum: &mut InstallAction, my_enum2: InstallAction) -> bool {
        my_enum != &my_enum2
    }
}

struct InstallationContext {
    engine: Engine,
    scope: Scope<'static>,
    config: Dynamic,
    script: Option<AST>,
    base_dir: PathBuf,
    cli: CommandLineArgs,
}

impl InstallationContext {
    fn new() -> anyhow::Result<Self> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let token = Alphanumeric.sample_string(&mut rng(), 20);

        let temp_install_dir = format!("_deploid_install_{now}_{token}");
        let base_dir = env::temp_dir().join(temp_install_dir);

        fs::create_dir(&base_dir)?;
        simplelog::WriteLogger::init(
            log::LevelFilter::Trace,
            Config::default(),
            File::create(base_dir.join("deploid_install.log")).unwrap(),
        )?;

        info!("Starting installation session");
        let mut ret = Self {
            engine: Engine::new(),
            scope: Scope::new(),
            config: Dynamic::from(Map::new()).into_shared(),
            script: None,
            base_dir,
            cli: CommandLineArgs::parse(),
        };

        ret.engine
            .register_type_with_name::<Step>("Step")
            .register_static_module("Step", exported_module!(StepModule).into())
            .register_type_with_name::<InstallAction>("InstallAction")
            .register_static_module(
                "InstallAction",
                exported_module!(InstallActionModule).into(),
            )
            .register_fn("load_text_from_file", load_text_from_file)
            .on_print(|s| {
                info!("{s}");
            })
            .on_debug(|t, s, p| {
                debug!("[{}:{p}]:{t}", s.unwrap_or("<default>"));
            });

        ret.insert_key_value_config("os".into(), std::env::consts::OS.into())?;
        ret.insert_key_value_config("arch".into(), std::env::consts::OS.into())?;
        ret.insert_key_value_config("dry_run".into(), ret.cli.dry_run.into())?;

        Ok(ret)
    }

    fn insert_key_value_config(&mut self, key: String, value: Dynamic) -> anyhow::Result<()> {
        self.config
            .as_map_mut()
            .map_err(|e| anyhow!("{e}"))
            .context("Cannot get the map from the value")?
            .insert(alias::String::from(key), value);
        Ok(())
    }

    fn get_config_value<'a>(&'a self, key: &str) -> anyhow::Result<Dynamic> {
        self.config
            .as_map_ref()
            .map_err(|e| anyhow!(e))?
            .get(key)
            .ok_or(anyhow!("Key '{key}' not found"))
            .cloned()
    }

    fn get_app_name(&self) -> anyhow::Result<String> {
        let ret = self
            .get_config_value("app_name")
            .context("failed to get app name")?
            .as_immutable_string_ref()
            .map_err(|e| anyhow!(e))
            .context("Cannot get app name as string")?
            .to_string();
        Ok(ret)
    }
    fn load_file(&mut self, file_name: &str) -> anyhow::Result<()> {
        self.script = Some(
            self.engine
                .compile_file(file_name.into())
                .map_err(|e| anyhow!("{e}"))?,
        );
        Ok(())
    }

    fn call_function<T: Clone + 'static>(
        &mut self,
        fn_name: &str,
        args: impl FuncArgs,
    ) -> anyhow::Result<T> {
        self.engine
            .call_fn(
                &mut self.scope,
                self.script.as_ref().expect("the script should be loaded"),
                fn_name,
                args,
            )
            .map_err(|e| anyhow!("{e}"))
    }
}

#[derive(Parser)]

struct CommandLineArgs {
    #[arg(
        short,
        long,
        default_value = "false",
        help = "Source folder where installation files lie."
    )]
    dry_run: bool,
}

fn main() -> anyhow::Result<()> {
    let mut ctx = InstallationContext::new()?;

    ratatui::run(|term| {
        let mut step = Step::Init;
        let mut history = vec![];
        loop {
            let res = step.act(term, &mut ctx);

            match res {
                Err(e) => {
                    error!("Error[{}]: {e}", line!());
                    let _ = act_error(term, &mut ctx, e);
                    return;
                }
                Ok(StepResult::Next) => {
                    let next = match ctx
                        .call_function("next_action", (ctx.config.clone(), step.clone()))
                    {
                        Ok(step) => step,
                        Err(e) => {
                            error!("Error[{}]: {e}", line!());
                            let _ = act_error(term, &mut ctx, e);
                            return;
                        }
                    };

                    if history.contains(&next) {
                        let _ = act_error(
                            term,
                            &mut ctx,
                            anyhow!("Loop detected !! step {next:?} already in the history.."),
                        );
                        return;
                    }

                    info!("Moving forward {next:?}..");
                    history.push(step);
                    step = next;
                }

                Ok(StepResult::Quit) => {
                    info!("Move to quit");
                    history.push(step);
                    step = Step::Quit;
                }

                Ok(StepResult::Finish) => {
                    info!("Exit..");
                    return;
                }

                Ok(StepResult::Back) => {
                    if let Some(s) = history.pop() {
                        info!("Moving back {s:?}..");
                        step = s;
                    } else {
                        error!("Stack is empty");
                        let _ = act_error(term, &mut ctx, Error::msg("The steps stack is empty"));
                        return;
                    }
                }
            }
        }
    });
    Ok(())
}
