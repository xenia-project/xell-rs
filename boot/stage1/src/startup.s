#define esr 		62
#define ivpr 		63
#define pid 		48
#define ctrlrd		136		
#define ctrlwr		152
#define pvr     	287
#define sprg0       272
#define hsprg0		304
#define hsprg1		305
#define hdsisr		306
#define hdar		307
#define dbcr0		308
#define dbcr1		309
#define hdec		310
#define hior 		311
#define rmor    	312
#define hrmor   	313
#define hsrr0		314
#define hsrr1		315
#define dac1		316
#define dac2		317
#define lpcr    	318
#define lpidr		319
#define tsr		    336
#define tcr		    340
#define tsrl		896
#define tsrr		897
#define tscr		921
#define ttr			922
#define PpeTlbIndexHint	946
#define PpeTlbIndex	947
#define PpeTlbVpn	948
#define PpeTlbRpn	949
#define PpeTlbRmt	951
#define dsr0		952
#define drmr0		953
#define dcidr0		954
#define drsr1		955
#define drmr1		956
#define dcidr1		957
#define issr0		976
#define irmr0		977
#define icidr0		978
#define irsr1		979
#define irmr1		980
#define icidr1		981
#define hid0    	1008
#define hid1		1009
#define hid4		1012
#define iabr    	1010
#define dabr    	1013     
#define dabrx		1015
#define buscsr  	1016
#define hid6    	1017
#define l2sr    	1018
#define BpVr		1022        
#define pir     	1023

.section .text.startup

.globl _start
_start:
    b	start_from_rom  // The CD loader will jump to this address.
    b	start_from_libxenon
    b	.	// for future use
    b	.
    b	.
    b	.
    b	.
    b	.

.section .text
.extern start_rust

// Startup XeLL from already-running OS (dashboard or libxenon)
start_from_libxenon:
	bl	init_regs
	li	%r4, 1
	b	start_common

// Startup XeLL from ROM.
.globl start_from_rom
start_from_rom:
	bl	init_regs
	li	%r4, 0

	// Intentional fallthrough.

// R1 = stack
// R2 = TOC
// R3 = clobber
// R4(store R30) = startup source
// R10 = clobber
// R11 = clobber
// R30 = clobber
start_common:
	mr		%r30, %r4			// Relocate startup source.
	mfspr   %r29, 318			// LPCR

	// disable MSR[EE]
	li		%r3, 2
	mtmsrd	%r3, 1

	li		%r3, 2
	isync
	mtspr	318, %r3            // LPCR[RMI] = 1 (Real-Mode cache inhibited)
	isync
	li      %r3, 0x3FF
	rldicr  %r3, %r3, 32,31
	tlbiel	%r3                  // TLB invalidate (local) 0x000003FF_00000000
	sync
	isync

	mfspr	%r10, 1009 // HID1
	li		%r11, 3
	rldimi	%r10, %r11, 58,4     // enable icache
	rldimi	%r10, %r11, 38,25    // instr. prefetch
	sync
	mtspr	1009, %r10 // HID1
	sync
	isync

	mfspr	%r10, 318 // LPCR
	li	    %r11, 1
	rldimi	%r10, %r11, 1,62
	isync
	mtspr	318, %r10 // LPCR
	isync

	bl 	disable_hrmor
	bl	relocate

    bl  load_toc

	mfspr	%r3, 1023 // PIR
	bl 	load_stack

	mfspr	%r3, 1023 // PIR
	cmplwi	%r3, 0
	bne		1f

	// Initialize BSS on processor 0 only.
	bl	init_bss

1:
	mfspr	%r3, 1023 // PIR
	mr		%r4, %r30 // Startup source.
	mfmsr	%r5
	mfspr	%r6, 313  // HRMOR
	mfpvr	%r7
	mr		%r8, %r29 // LPCR

	bl 		__start_rust
	ori		%r0, %r0, 0

	b	.

// Initialize hardware registers.
// R3 = clobber
init_regs:
	or	%r2, %r2, %r2 // normal priority

	// Set up the HID (Hardware Implementation Dependent) registers.
	// Refer to Cell Broadband Engine Registers, v1.5

	// HID0: Implementation differs per CPU, but some bits are reused.
	// On the Cell Broadband Engine, this just inhibits things we probably don't want.
	li	%r3, 0
	mtspr	1008, %r3 // HID0
	sync
	isync

	// As per the Cell Broadband Engine Hardware Initialization Guide.
	// Enable the L1 data cache.
	// 0x00003F0000000000
	li	%r3, 0x3f00
	rldicr	%r3, %r3, 32,31
	mtspr	1012, %r3 // HID4
	sync
	isync

	// As per Cell Broadband Engine Hardware Initialization Guide.
	// Enable the L1 instruction cache, and make 0x100 the reset vector for thread 0.
	// DIS_SYSRST_REG = 1 (Disable config ring system reset vector)
	// 0x9C30104000000000
	lis	%r3, 0x9c30
	ori	%r3,%r3, 0x1040
	rldicr	%r3, %r3, 32,31
	mtspr   1009, %r3 // HID1
	sync
	isync

	// Initialize RMSC to set the real mode address boundary to 2TB.
	// RMSC = 0b1110b
	// LB   = 0b1000 (64KB / 16MB large page table size)
	// TB   = 0b1 (Time base enabled)
	// 0x0001803800000000
	lis	%r3, 1
	ori	%r3,%r3, 0x8038
	rldicr	%r3, %r3, 32,31
	mtspr	1017, %r3 // HID6
	sync
	isync

	// Thread Switch Control Register (tscr)
	// WEXT = 1
	// PBUMP = 1 (boost thread priority level to medium when interrupt pending)
	// FPCF = 1
	// PSCTP = 1 (privileged can change priority)
	// 0x001D0000
	lis	%r3, 0x1d
	mtspr	921, %r3 // TSCR
	sync
	isync

	// Thread Switch Timeout Register
	// TTM = 0x1000 (thread interrupted after executing 4096 instructions)
	li	%r3, 0x1000
	mtspr	922, %r3 // TTR
	sync
	isync

	blr

// Initialize BSS
// R10 = clobber
// R11 = clobber
// CTR = clobber
init_bss:
	ld		%r10, __bss_start@toc(%r2)
	ld		%r11, __bss_end@toc(%r2)
	sub		%r11, %r11, %r10 // r11 = (end - start)
	srdi	%r11, %r11, 2    // r11 /= 4
	subi	%r10, %r10, 4
	cmplwi	%r11, 0
	beq		1f

	mtctr	%r11
	li		%r11, 0

.bss_loop:
	stwu	%r11, 4(%r10)
	bdnz	.bss_loop

1:
	blr

// Sets the high bit in PC, disabling HRMOR.
// R3 = clobber
// R10 = clobber
disable_hrmor:
    mflr    %r10

    lis	    %r3, 0x8000
    sldi	%r3, %r3, 32
    or      %r3, %r3, %r10

	mtlr	%r3
	blr

// Sets up the stack.
// R1(out) = stack
// R3(in/out) = pir / clobber
load_stack:
	// set stack
	// R1 = 0x80000000_1E000000
	lis		%r1, 0x8000
	rldicr	%r1, %r1, 32,31
	oris	%r1, %r1, 0x1e00
	
	slwi	%r3, %r3, 16  		 // 64k stack per thread
	sub		%r1, %r1, %r3
	subi	%r1, %r1, 0x80
	blr

// R3 = lr
load_lr:
	mflr	%r3
	blr

// Loads the table of contents pointer into R2.
// R0 = clobber
// R2 = TOC
// R3 = clobber
load_toc:
    mflr    %r0
    bcl     20, 31, $+4
0:  
    mflr    %r3
    ld      %r2, (p_toc - 0b)(%r3)
    add     %r2, %r2, %r3
    mtlr    %r0
    blr

.balign 8
p_toc:  .8byte  __toc_start + 0x8000 - 0b

// Relocates the bootloader to the compiled-in address.
// R2 = TOC
// R3 = clobber
// R4 = clobber
// R5 = clobber
// R6 = clobber
// R7 = clobber
// R10 = clobber
// R11 = clobber
relocate:
	mflr 	%r10

	// Load the TOC.
	bl		load_toc

	bl		load_lr
0:

	// Relocate the relocation routine to 0x8000_0000_0000_0000.
	addi	%r4, %r3, __relocate_memmove_start - 0b
	lis		%r3, 0x8000
	sldi	%r3, %r3, 32
	mr		%r11, %r3
	li		%r5, __relocate_memmove_end - __relocate_memmove_start
	bl		relocate_memmove

	// Great. Now relocate the bootloader.
	lis		%r3, (__toc_start + 0x8000)@highest
	ori		%r3, %r3, (__toc_start + 0x8000)@higher
	sldi	%r3, %r3, 32
	oris	%r3, %r3, (__toc_start + 0x8000)@high
	ori		%r3, %r3, (__toc_start + 0x8000)@l

	// R4 = move delta
	sub		%r4, %r3, %r2

	// Relocate the return address.
	add		%r10, %r10, %r4

	// R3 = DST
	lis		%r3, _start@highest
	ori		%r3, %r3, _start@higher
	sldi	%r3, %r3, 32
	oris	%r3, %r3, _start@high
	ori		%r3, %r3, _start@l

	// R4 = SRC
	sub		%r4, %r3, %r4

	// R5 = LEN
	lis		%r5, 0x1

	// Restore the return address.
	mtlr	%r10
	mtctr	%r11
	bctr

__relocate_memmove_start:

// R3 = dst
// R4 = src
// R5 = len
// R6 = clobber
// R7 = clobber
relocate_memmove:
	cmpld	%r3, %r4
	bge 	forward

backward:
	add		%r4, %r4, %r5
	addi	%r6, %r5, 1
	add		%r5, %r3, %r5
	mtctr	%r6

backward_loop:
	bdzlr
	lbz		%r6, -0x1(%r4)
	subi	%r7, %r5, 1
	subi	%r4, %r4, 1
	stb		%r6, -0x1(%r5)
	mr		%r5, %r7
	b		backward_loop

forward:
	subi	%r4, %r4, 1
	addi	%r6, %r5, 1
	subi	%r5, %r3, 1
	mtctr	%r6
	bdzlr

forward_loop:
	lbz		%r6, 0x1(%r4)
	addi	%r7, %r5, 1
	addi	%r4, %r4, 1
	stb		%r6, 0x1(%r5)
	mr		%r5, %r7
	bdnz	forward_loop

	blr

__relocate_memmove_end:
