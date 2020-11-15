//! Support for the 8259 Programmable Interrupt Controller, which handles
//! basic I/O interrupts.  In multicore mode, we would apparently need to
//! replace this with an APIC interface.
//!
//! The basic idea here is that we have two PIC chips, PIC1 and PIC2, and
//! that PIC2 is slaved to interrupt 2 on PIC 1.  You can find the whole
//! story at http://wiki.osdev.org/PIC (as usual).  Basically, our
//! immensely sophisticated modern chipset is engaging in early-80s
//! cosplay, and our goal is to do the bare minimum required to get
//! reasonable interrupts.
//!
//! The most important thing we need to do here is set the base "offset"
//! for each of our two PICs, because by default, PIC1 has an offset of
//! 0x8, which means that the I/O interrupts from PIC1 will overlap
//! processor interrupts for things like "General Protection Fault".  Since
//! interrupts 0x00 through 0x1F are reserved by the processor, we move the
//! PIC1 interrupts to 0x20-0x27 and the PIC2 interrupts to 0x28-0x2F.  If
//! we wanted to write a DOS emulator, we'd presumably need to choose
//! different base interrupts, because DOS used interrupt 0x21 for system
//! calls.

#![feature(const_fn)]
#![no_std]

extern crate cpuio;

/// Command sent to begin PIC initialization.
const CMD_INIT: u8 = 0x11;

/// Command sent to acknowledge an interrupt.
const CMD_END_OF_INTERRUPT: u8 = 0x20;

// The mode in which we want to run our PICs.
const MODE_8086: u8 = 0x01;

// In-Service Register
const CMD_READ_ISR: u8 = 0x0b;

/// An individual PIC chip.  This is not exported, because we always access
/// it through `Pics` below.
struct Pic {
    /// The base offset to which our interrupts are mapped.
    offset: u8,

    /// The processor I/O port on which we send commands.
    command: cpuio::UnsafePort<u8>,

    /// The processor I/O port on which we send and receive data.
    data: cpuio::UnsafePort<u8>,
    orig_mask: Option<u8>,
}

impl Pic {
    /// Are we in change of handling the specified interrupt?
    /// (Each PIC handles 8 interrupts.)
    fn handles_interrupt(&self, interupt_id: u8) -> bool {
        self.offset <= interupt_id && interupt_id < self.offset + 8
    }

    /// Notify us that an interrupt has been handled and that we're ready
    /// for more.
    unsafe fn end_of_interrupt(&mut self) {
        self.command.write(CMD_END_OF_INTERRUPT);
    }
}

/// A pair of chained PIC controllers.  This is the standard setup on x86.
pub struct ChainedPics {
    pics: [Pic; 2],
}

impl ChainedPics {
    /// Create a new interface for the standard PIC1 and PIC2 controllers,
    /// specifying the desired interrupt offsets.
    pub const unsafe fn new(offset1: u8, offset2: u8) -> ChainedPics {
        ChainedPics {
            pics: [
                Pic {
                    offset: offset1,
                    command: cpuio::UnsafePort::new(0x20),
                    data: cpuio::UnsafePort::new(0x21),
                    orig_mask: None,
                },
                Pic {
                    offset: offset2,
                    command: cpuio::UnsafePort::new(0xA0),
                    data: cpuio::UnsafePort::new(0xA1),
                    orig_mask: None,
                },
            ],
        }
    }

    /// Initialize both our PICs.  We initialize them together, at the same
    /// time, because it's traditional to do so, and because I/O operations
    /// might not be instantaneous on older processors.
    pub unsafe fn initialize(&mut self) {
        // We need to add a delay between writes to our PICs, especially on
        // older motherboards.  But we don't necessarily have any kind of
        // timers yet, because most of them require interrupts.  Various
        // older versions of Linux and other PC operating systems have
        // worked around this by writing garbage data to port 0x80, which
        // allegedly takes long enough to make everything work on most
        // hardware.  Here, `wait` is a closure.
        let mut wait_port: cpuio::Port<u8> = cpuio::Port::new(0x80);
        let mut wait = || wait_port.write(0);

        // Save our original interrupt masks, because I'm too lazy to
        // figure out reasonable values.  We'll restore these when we're
        // done.
        let saved_mask1 = self.pics[0].data.read();
        let saved_mask2 = self.pics[1].data.read();

        // Tell each PIC that we're going to send it a three-byte
        // initialization sequence on its data port.
        self.pics[0].command.write(CMD_INIT);
        wait();
        self.pics[1].command.write(CMD_INIT);
        wait();

        // Byte 1: Set up our base offsets.
        self.pics[0].data.write(self.pics[0].offset);
        wait();
        self.pics[1].data.write(self.pics[1].offset);
        wait();

        // Byte 2: Configure chaining between PIC1 and PIC2.
        self.pics[0].data.write(4);
        wait();
        self.pics[1].data.write(2);
        wait();

        // Byte 3: Set our mode.
        self.pics[0].data.write(MODE_8086);
        wait();
        self.pics[1].data.write(MODE_8086);
        wait();

        // Restore our saved masks.
        self.pics[0].data.write(saved_mask1);
        self.pics[1].data.write(saved_mask2);
    }

    pub unsafe fn mask_all(&mut self) {
        let pic0 = &mut self.pics[0];
        pic0.orig_mask = Some(pic0.data.read());
        pic0.data.write(0xff);

        let pic1 = &mut self.pics[1];
        pic1.orig_mask = Some(pic1.data.read());
        pic1.data.write(0xff);
    }

    // 0 means interrupt enabled, 1 means masked
    pub unsafe fn mask(&mut self, pic1_mask:u8, pic2_mask:u8){
        let pic0 = &mut self.pics[0];
        pic0.data.write(pic1_mask);

        let pic1 = &mut self.pics[1];
        pic1.data.write(pic2_mask);
    }

    /* PIC2 is chained, and
     * represents IRQs 8-15.  PIC1 is IRQs 0-7, with 2 being the chain */
    // Execute closure f if this is a spurious interrupt afterwards
    // send end of interrupt (eio) if irq > 7
    pub unsafe fn is_spurious_interrupt<F: FnOnce()>(&mut self, f: F) -> bool {
        let pic1 = &mut self.pics[0];
        pic1.command.write(CMD_READ_ISR);
        let irq = pic1.command.read();
        // If IRQ2 is set then check slave controller
        if irq & (1 << 2) == 1 {
            let pic2 = &mut self.pics[0];
            pic2.command.write(CMD_READ_ISR);
            let irq = pic2.command.read();

            if irq == 0 {
                f();
                // Send eio to pic1 because it does not know
                // that this is only a spurious interrupt
                self.pics[0].end_of_interrupt();
                return true;
            }
        } else {
            // if IRQ2 (bit 2) is not set then irq should equal to 0 if
            // it is a spurious interrupt.
            if irq == 0 {
                f();
                return true;
            }
        }
        return false;
    }

    /// Do we handle this interrupt?
    pub fn handles_interrupt(&self, interrupt_id: u8) -> bool {
        self.pics.iter().any(|p| p.handles_interrupt(interrupt_id))
    }

    /// Figure out which (if any) PICs in our chain need to know about this
    /// interrupt.  This is tricky, because all interrupts from `pics[1]`
    /// get chained through `pics[0]`.
    pub unsafe fn notify_end_of_interrupt(&mut self, interrupt_id: u8) {
        if self.handles_interrupt(interrupt_id) {
            if self.pics[1].handles_interrupt(interrupt_id) {
                self.pics[1].end_of_interrupt();
            }
            self.pics[0].end_of_interrupt();
        }
    }
}
