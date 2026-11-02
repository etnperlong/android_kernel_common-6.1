SECTIONS {
	.initcall6.init : { KEEP(*(.initcall6.init)) }
	__param : { KEEP(*(__param)) }
	.modinfo : { KEEP(*(.modinfo)) }
	.altinstructions : { KEEP(*(.altinstructions)) }
	__jump_table : { KEEP(*(__jump_table)) }
	__bug_table : { KEEP(*(__bug_table)) }
	.discard.addressable : { KEEP(*(.discard.addressable)) }
	.discard.ksym : { KEEP(*(.discard.ksym)) }
	.export_symbol : { KEEP(*(.export_symbol)) }
	__ksymtab_strings : { KEEP(*(__ksymtab_strings)) }
	___ksymtab_gpl+wrapfd_acquire_ownership : { KEEP(*(___ksymtab_gpl+wrapfd_acquire_ownership)) }
	___ksymtab_gpl+wrapfd_add_listener : { KEEP(*(___ksymtab_gpl+wrapfd_add_listener)) }
	___ksymtab_gpl+wrapfd_del_listener : { KEEP(*(___ksymtab_gpl+wrapfd_del_listener)) }
	___ksymtab_gpl+wrapfd_get_mappable : { KEEP(*(___ksymtab_gpl+wrapfd_get_mappable)) }
	___ksymtab_gpl+wrapfd_put_mappable : { KEEP(*(___ksymtab_gpl+wrapfd_put_mappable)) }
	___ksymtab_gpl+wrapfd_release_ownership : { KEEP(*(___ksymtab_gpl+wrapfd_release_ownership)) }
	.note.gnu.property : { KEEP(*(.note.gnu.property)) }
	.text : { *(.text .text.*) }
	.rodata : { *(.rodata .rodata.*) }
	.data : { *(.data .data.*) }
	.bss : { *(.bss .bss.*) }
}
