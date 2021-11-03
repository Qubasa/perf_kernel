# Glue gun

Configuration is done through a through a `[package.metadata.glue_gun]`
 table in the Cargo.toml of your kernel. The following options are available:

```toml
[package.metadata.glue_gun]
# The command invoked with the created glue_gun (the "{}" will be replaced
# with the path to the bootable disk image)
# Applies to `glue_gun run`
run-command = ["qemu-system-x86_64", "-drive", "format=raw,file={}"]

# Additional arguments passed to the run command for non-test executables
# Applies to `glue_gun run
run-args = []

# Additional arguments passed to the run command for test executables
# Applies to `glue_gun run`
test-args = []

# An exit code that should be considered as success for test executables
test-success-exit-code = {integer}

# The timeout for running a test through `glue_gun test` or `glue_gun runner` (in seconds)
test-timeout = 300

# Whether the `-no-reboot` flag should be passed to test executables
test-no-reboot = true
```