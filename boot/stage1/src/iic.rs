const IIC_BASE: usize = 0x80000200_00050000;

//! Integrated Interrupt Controller (IIC)
#[repr(usize)]
enum Register {
    WhoAmI = 0x00,
    CurrentTaskPriority = 0x08,
    IpiDispatch = 0x10,
    Ack = 0x50,
    AckPri = 0x58,
    Eoi = 0x60,
    EoiPri = 0x68,

    Unk70 = 0x70,
}

