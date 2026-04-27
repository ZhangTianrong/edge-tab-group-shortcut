# Get the absolute path of the project root (Windows-compatible)
ROOT_DIR := $(CURDIR)

# Build targets
.PHONY: all clean build install test uninstall package-store package-companion

all: build

build: build-host build-detector

build-host:
	@echo Building native host...
	cd native-host && cargo build --release

build-detector:
	@echo Building hover detector...
	cd hover-detector && cargo build --release

clean:
	@echo Cleaning build artifacts...
	cd native-host && cargo clean
	cd hover-detector && cargo clean

install: build
	@echo Installing Windows companion files...
	powershell -NoProfile -ExecutionPolicy Bypass -File install.ps1 -Channel development

uninstall:
	@echo Uninstalling Windows companion files...
	powershell -NoProfile -ExecutionPolicy Bypass -File uninstall.ps1

package-store:
	@echo Packaging Edge Store submission...
	powershell -NoProfile -ExecutionPolicy Bypass -File scripts\\package-edge-store.ps1

package-companion: build
	@echo Packaging Windows companion bundle...
	powershell -NoProfile -ExecutionPolicy Bypass -File scripts\\package-windows-companion.ps1

test: build
	@echo Running native host test...
	cd native-host && node test_native_host.js
