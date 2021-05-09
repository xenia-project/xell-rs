#define esr 		62
#define ivpr 		63
#define pid 		48
#define ctrlrd		136		
#define ctrlwr		152
#define pvr     	287
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
// R4 = startup source
// R10 = clobber
// R11 = clobber
start_common:

	// disable interrupts (but enable vector available, gcc likes to use VMX
	// for memset)
	lis		%r3, 0x200
	mtmsrd	%r3, 1

	li		%r3, 2
	isync
	mtspr	lpcr, %r3            // LPCR[RMI] = 1 (Real-Mode cache inhibited)
	isync
	li      %r3, 0x3FF
	rldicr  %r3, %r3, 32,31
	tlbiel	%r3                  // TLB invalidate (local) 0x000003FF_00000000
	sync
	isync

	mfspr	%r10, hid1
	li		%r11, 3
	rldimi	%r10, %r11, 58,4     // enable icache
	rldimi	%r10, %r11, 38,25    // instr. prefetch
	sync
	mtspr	hid1, %r10
	sync
	isync

	mfspr	%r10, lpcr
	li	    %r11, 1
	rldimi	%r10, %r11, 1,62
	isync
	mtspr	lpcr, %r10
	isync

	// set stack
	// R1 = 0x80000000_1E000000
	li	%r1, 0
	oris	%r1, %r1, 0x8000
	rldicr	%r1, %r1, 32,31
	oris	%r1, %r1, 0x1e00
	
1:
	slwi	%r3, %r3, 16  		 // 64k stack per thread
	sub		%r1, %r1, %r3
	subi	%r1, %r1, 0x80

	// lis	%r3, 0x8000
	// rldicr  %r3, %r3, 32,31
	// oris	%r3, %r3, start@high
	// ori	%r3, %r3, start@l
	// ld	%r2, 8(%r3)

    // Set the high bit for PC.
    bl      1f
1:
    mflr    %r10

    lis	    %r3, 0x8000
    sldi	%r3, %r3, 32
    or      %r3, %r3, %r10
    addi    %r3, %r3, (1f - 1b)
    mtctr   %r3
    bctr

1:
    bl  load_toc

	mfspr	%r3, pir
	cmplwi	%r3, 0
	bne		1f

	// Initialize BSS.
	ld		%r10, __bss_start@got(%r2)
	ld		%r11, __bss_end@got(%r2)
	sub		%r11, %r11, %r10
	srdi	%r11, %r11, 2
	subi	%r10, %r10, 1
	cmplwi	%r11, 0
	beq		1f

	mtctr	%r11
	li		%r11, 0

.bss_loop:
	stwu	%r11, 4(%r10)
	bdnz	.bss_loop

1:
	// Relocate startup source.
	mr		%r6, %r4

	mfspr	%r3, pir
	mfspr	%r4, hrmor
	mfpvr	%r5

	// Branch to start_rust via 64-bit ELF ABI.
	ld		%r0, __start_rust@got(%r2)
	ld		%r0, 0(%r0)
	mtctr	%r0
	bctrl
	nop

	b	.

.globl other_threads_startup
other_threads_startup:
	mfspr	%r3, pir
	andi.   %r3,%r3,1
	cmplwi  %r3,1
	beq	1f

	bl	init_regs
	
	li		%r3,0
	mtspr	hrmor,%r3
	sync
	isync

	// 0x00C00000
	// TE = 0b11 (enable both threads)
	lis		%r3,0xC0
	mtspr	ctrlwr,%r3
	sync
	isync

1:
	li	    %r4,0x30 // Clear IR/DR
	mfmsr	%r3
	andc	%r3,%r3,%r4
	mtsrr1	%r3

    // Branch to the startup routine.
    // 0x80000000_1C000000
    lis     %r3, 0x8000
    rldicr  %r3, %r3, 32, 31
    oris    %r3, %r3, 0x1C00

	mtsrr0	%r3
	rfid

// Initialize hardware registers.
// R3 = clobber
init_regs:
	or	%r2, %r2, %r2 // normal priority

	// Set up the HID (Hardware Implementation Dependent) registers.
	// Refer to Cell Broadband Engine Registers, v1.5

	// HID0: Implementation differs per CPU, but some bits are reused.
	// On the Cell Broadband Engine, this just inhibits things we probably don't want.
	li	%r3, 0
	mtspr	hid0, %r3
	sync
	isync

	// As per the Cell Broadband Engine Hardware Initialization Guide.
	// Enable the L1 data cache.
	// 0x00003F0000000000
	li	%r3, 0x3f00
	rldicr	%r3, %r3, 32,31
	mtspr	hid4, %r3
	sync
	isync

	// As per Cell Broadband Engine Hardware Initialization Guide.
	// Enable the L1 instruction cache, and make 0x100 the reset vector for thread 0.
	// DIS_SYSRST_REG = 1 (Disable config ring system reset vector)
	// 0x9C30104000000000
	lis	%r3, 0x9c30
	ori	%r3,%r3, 0x1040
	rldicr	%r3, %r3, 32,31
	mtspr   hid1, %r3
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
	mtspr	hid6, %r3
	sync
	isync

	// Thread Switch Control Register (tscr)
	// WEXT = 1
	// PBUMP = 1 (boost thread priority level to medium when interrupt pending)
	// FPCF = 1
	// PSCTP = 1 (privileged can change priority)
	// 0x001D0000
	lis	%r3, 0x1d
	mtspr	tscr, %r3
	sync
	isync

	// Thread Switch Timeout Register
	// TTM = 0x1000 (thread interrupted after executing 4096 instructions)
	li	%r3, 0x1000
	mtspr	ttr, %r3
	sync
	isync

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

.globl other_threads_startup_end
other_threads_startup_end:

