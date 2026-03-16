
BUILD=dev
.PHONY: clean

template.exe: template/src/main.rs
	@ echo "Building template.exe"
	@ cargo.exe build -p template --profile $(BUILD)

deploid.exe: template.exe deploid/src/main.rs
	@ echo "Building deploid.exe"
	@ cargo.exe build -p deploid --profile $(BUILD)

clean:
	@ echo "Cleaning up..."
	cargo.exe clean

all: template.exe deploid.exe

