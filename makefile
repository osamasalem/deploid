
BUILD=dev
.PHONY: clean

installer.exe: installer/src/main.rs installer/Cargo.toml
	@ echo "Building installer.exe"
	@ cargo.exe build -p installer --profile $(BUILD)

deploid.exe: installer.exe deploid/src/main.rs deploid/Cargo.toml
	@ echo "Building deploid.exe"
	@ cargo.exe build -p deploid --profile $(BUILD)

clean:
	@ echo "Cleaning up..."
	cargo.exe clean

all: installer.exe deploid.exe

