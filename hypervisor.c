#include <linux/init.h>
#include <linux/module.h>

MODULE_AUTHOR("Qubasa Corp.");
MODULE_LICENSE("GPL v2");

enum SVM_SUPPORT {
  SVM_ALLOWED,
  SVM_NOT_AVAIL,
  SVM_DISABLED_AT_BIOS_NOT_UNLOCKABLE,
  SVM_DISABLED_WITH_KEY
};

bool hasMsrSupport(void) {
  uint32_t cpuid_response;

  // Get CPUID for MSR support
  __asm__("mov rax, 0x00000001":::"rax");
  __asm__("cpuid");
  __asm__("mov %0, edx" : "=r"(cpuid_response));

  if (cpuid_response & (1 << 5)) {
    return true;
  }
  return false;
}

void readMSR(uint32_t id, uint32_t *hi, uint32_t *lo) {
  __asm__("rdmsr" : "=a"(*lo), "=d"(*hi) : "c"(id));
}

bool isSVMDisabled(void) {
  uint32_t VM_CR;
  uint32_t high;

  // Read VM_CR MSR
  readMSR(0xC0010114, &high, &VM_CR);

  return (bool)(VM_CR & (1 << 4));
}

enum SVM_SUPPORT hasSvmSupport(void) {
  uint32_t cpuid_response;

  // Get CPUID for svm support
  __asm__("mov rax, 0x80000001":::"rax");
  __asm__("cpuid");
  __asm__("mov %0, ecx" : "=r"(cpuid_response));

  // Has SVM extension?
  if (!(cpuid_response & 0x2)) {
    return SVM_NOT_AVAIL;
  }

  if(!isSVMDisabled()){
    return SVM_ALLOWED;
  }

  // Get CPUID for disabled svm at bios
  __asm__("mov rax, 0x8000000A":::"rax");
  __asm__("cpuid");
  __asm__("mov %0, edx" : "=r"(cpuid_response));

  // Check if SVM is disabled in BIOS
  if ((cpuid_response & 0x2) == 0) {
    return SVM_DISABLED_AT_BIOS_NOT_UNLOCKABLE;
  } else {
    return SVM_DISABLED_WITH_KEY;
  }
}

static int my_init(void) {
  enum SVM_SUPPORT svm;
  printk(KERN_INFO "==== LOADED HYPERVISOR DRIVER ====\n");

  if (!hasMsrSupport()) {
    printk(KERN_INFO "System does not have MSR support\n");
    return 1;
  }

  svm = hasSvmSupport();

  switch (svm) {
  case SVM_ALLOWED:
    printk(KERN_INFO "Has SVM support: true\n");
    break;
  case SVM_NOT_AVAIL:
    printk(KERN_INFO "Has SVM support: false\n");
    return 1;
  case SVM_DISABLED_WITH_KEY:
    printk(KERN_INFO "SVM is bios disabled with key\n");
    return 1;
  case SVM_DISABLED_AT_BIOS_NOT_UNLOCKABLE:
    printk(KERN_INFO "SVM is bios disabled not unlockable\n");
    return 1;
  }
  return 0;
}

static void my_exit(void) {
  printk(KERN_INFO "Goodbye world.\n");

  return;
}

module_init(my_init);
module_exit(my_exit);
