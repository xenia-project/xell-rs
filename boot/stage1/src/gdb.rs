use core::num::NonZeroUsize;

use gdbstub::{
    target::{
        ext::base::multithread::{MultiThreadOps, ThreadStopReason},
        Target,
    },
    GdbStubBuilder,
};
use gdbstub_arch::ppc;

struct BLTarget {}

impl Target for BLTarget {
    type Arch = ppc::PowerPcAltivec32<()>;
    type Error = &'static str;

    fn base_ops(&mut self) -> gdbstub::target::ext::base::BaseOps<Self::Arch, Self::Error> {
        gdbstub::target::ext::base::BaseOps::MultiThread(self)
    }
}

impl MultiThreadOps for BLTarget {
    fn resume(
        &mut self,
        _default_resume_action: gdbstub::target::ext::base::ResumeAction,
        _gdb_interrupt: gdbstub::target::ext::base::GdbInterrupt<'_>,
    ) -> Result<
        gdbstub::target::ext::base::multithread::ThreadStopReason<
            <Self::Arch as gdbstub::arch::Arch>::Usize,
        >,
        Self::Error,
    > {
        // Does nothing for now...
        Ok(ThreadStopReason::DoneStep)
    }

    fn clear_resume_actions(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_resume_action(
        &mut self,
        _tid: gdbstub::common::Tid,
        _action: gdbstub::target::ext::base::ResumeAction,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn read_registers(
        &mut self,
        _regs: &mut <Self::Arch as gdbstub::arch::Arch>::Registers,
        _tid: gdbstub::common::Tid,
    ) -> gdbstub::target::TargetResult<(), Self> {
        Ok(())
    }

    fn write_registers(
        &mut self,
        _regs: &<Self::Arch as gdbstub::arch::Arch>::Registers,
        _tid: gdbstub::common::Tid,
    ) -> gdbstub::target::TargetResult<(), Self> {
        Ok(())
    }

    fn read_addrs(
        &mut self,
        start_addr: <Self::Arch as gdbstub::arch::Arch>::Usize,
        data: &mut [u8],
        tid: gdbstub::common::Tid,
    ) -> gdbstub::target::TargetResult<(), Self> {
        let src = unsafe { core::slice::from_raw_parts(start_addr as *const _, data.len()) };

        Ok(())
    }

    fn write_addrs(
        &mut self,
        start_addr: <Self::Arch as gdbstub::arch::Arch>::Usize,
        data: &[u8],
        tid: gdbstub::common::Tid,
    ) -> gdbstub::target::TargetResult<(), Self> {
        Ok(())
    }

    fn list_active_threads(
        &mut self,
        thread_is_active: &mut dyn FnMut(gdbstub::common::Tid),
    ) -> Result<(), Self::Error> {
        thread_is_active(NonZeroUsize::new(1).unwrap());

        Ok(())
    }
}

pub fn entry(uart: &mut crate::uart::UART) {
    let mut target = BLTarget {};

    let mut buf = [0u8; 4096];
    let mut gdb = GdbStubBuilder::new(uart)
        .with_packet_buffer(&mut buf)
        .build()
        .unwrap();

    match gdb.run(&mut target) {
        Ok(_) => {}
        Err(_) => {}
    }
}
