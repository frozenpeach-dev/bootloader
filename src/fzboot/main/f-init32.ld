ENTRY(_entry32)

SECTIONS {

    . = 0x5f00 + 0x7c00;

    .start : {
        *(.start)
    }

    .interrupts : {
        KEEP(*(.int*))
    }

    .text : {
        *(.text .text.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .data : {
        *(.data .data.*)
    }

    _bss_start = .;
    .bss : {
        *(.bss .bss.*)
    }
    _bss_end = .;

    .eh_frame : {
        *(.eh_frame .eh_frame.*)
    }

    .eh_frame_hdr : {
        *(.eh_frame_hdr .eh_frame_hdr.*)
    }

    .fill : {
            FILL(0xdeadc0de);
            . = 0x100000;
        }

}
