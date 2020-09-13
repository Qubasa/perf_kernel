## Get started
Compile with
```bash
$ make
$ insmod hypervisor.ko
```

See logs with
```
$ dmesg --follow
```

## Resources
Look into `linux/arch/x86/kvm/svm.c`

Interesting functions are:
* pre_svm_run
* pre_sev_run
