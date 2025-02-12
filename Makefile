# Get the absolute path of the project root (Windows-compatible)
ROOT_DIR := $(CURDIR)

# Build targets
.PHONY: all clean build install test uninstall

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

# Generate native messaging host manifest with correct paths
native-messaging-host.json: native-host/target/release/native-host.exe
	@echo Generating native messaging host manifest...
	@echo { > $@
	@echo   "name": "com.tabgroup.shortcut", >> $@
	@echo   "description": "Native messaging host for TabGroup Keyboard Shortcuts extension", >> $@
	@echo   "path": "$(subst /,\\,$(ROOT_DIR))\\native-host\\target\\release\\native-host.exe", >> $@
	@echo   "type": "stdio", >> $@
	@echo   "allowed_origins": [ >> $@
	@echo     "chrome-extension://kndnhaaahebnocoeepehnccdkkcbeegk/" >> $@
	@echo   ] >> $@
	@echo } >> $@

# Generate registry files with correct paths
register_host.reg: native-messaging-host.json
	@echo Generating registry file...
	@echo Windows Registry Editor Version 5.00 > $@
	@echo. >> $@
	@echo [HKEY_CURRENT_USER\Software\Google\Chrome\NativeMessagingHosts\com.tabgroup.shortcut] >> $@
	@echo @="$(subst /,\\,$(ROOT_DIR))\\native-messaging-host.json" >> $@
	@echo. >> $@
	@echo [HKEY_CURRENT_USER\Software\Microsoft\Edge\NativeMessagingHosts\com.tabgroup.shortcut] >> $@
	@echo @="$(subst /,\\,$(ROOT_DIR))\\native-messaging-host.json" >> $@

unregister_host.reg:
	@echo Generating unregister file...
	@echo Windows Registry Editor Version 5.00 > $@
	@echo. >> $@
	@echo [-HKEY_CURRENT_USER\Software\Google\Chrome\NativeMessagingHosts\com.tabgroup.shortcut] >> $@
	@echo [-HKEY_CURRENT_USER\Software\Microsoft\Edge\NativeMessagingHosts\com.tabgroup.shortcut] >> $@

install: build native-messaging-host.json register_host.reg
	@echo Installing native messaging host...
	reg import register_host.reg
	@echo Installation complete. Please restart your browser.

uninstall: unregister_host.reg
	@echo Uninstalling native messaging host...
	reg import unregister_host.reg
	@echo Uninstallation complete.

test: build
	@echo Running native host test...
	cd native-host && node test_native_host.js
