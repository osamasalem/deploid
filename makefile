
BUILD=dev
.PHONY: clean

installer.exe: installer/src/main.rs installer/Cargo.toml
	@ echo "Building installer.exe"
	@ cargo.exe build -p installer --profile $(BUILD)

deploid.exe: installer.exe deploid/src/main.rs deploid/Cargo.toml
	@ echo "Building deploid.exe"
	@ cargo.exe build -p deploid --profile $(BUILD)

example: installer.exe deploid.exe
	@ cargo.exe run -p deploid --profile $(BUILD) -- --output deploid-example.exe --source example

clean:
	@ echo "Cleaning up..."
	cargo.exe clean

all: example

