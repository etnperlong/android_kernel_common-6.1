SECTIONS {
	.initcall6.init : { KEEP(*(.initcall6.init)) }
	__param : { KEEP(*(__param)) }
	.modinfo : { KEEP(*(.modinfo)) }
	.text : { *(.text .text.*) }
	.rodata : { *(.rodata .rodata.*) }
	.data : { *(.data .data.*) }
	.bss : { *(.bss .bss.*) }
}
