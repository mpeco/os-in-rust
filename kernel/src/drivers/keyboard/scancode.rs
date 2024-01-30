static KEY_TO_CHAR: [Option<&'static str>; IbmXt::KeypadPeriod as usize] = [
    None,
    Some("1"), Some("2"), Some("3"), Some("4"), Some("5"), Some("6"), Some("7"), Some("8"), Some("9"), Some("0"),
    Some("-"), Some("="), None, None,
    Some("q"), Some("w"), Some("e"), Some("r"), Some("t"), Some("y"), Some("u"), Some("i"), Some("o"), Some("p"),
    Some("["), Some("]"), Some("\n"), None,
    Some("a"), Some("s"), Some("d"), Some("f"), Some("g"), Some("h"), Some("j"), Some("k"), Some("l"),
    Some(";"), Some("'"), Some("`"), None, Some("\\"),
    Some("z"), Some("x"), Some("c"), Some("v"), Some("b"), Some("n"), Some("m"),
    Some(","), Some("."), Some("/"), None, Some("*"), None, Some(" "), None,
    None, None, None, None, None, None, None, None, None, None,
    None, None,
    Some("7"), Some("8"), Some("9"), Some("-"), Some("4"), Some("5"), Some("6"), Some("+"),
    Some("1"), Some("2"), Some("3"), Some("0"), Some("."),
];

const KEY_RELEASED: u8 = 0x80;

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum IbmXt {
    // PRESSED:
    Esc = 0x1,
    One, Two, Three, Four, Five, Six, Seven, Eigth, Nine, Zero,
    Minus, Equal, Backspace, Tab,
    Q, W, E, R, T, Y, U, I, O, P,
    OpenBracket, CloseBracket, Enter, LCtrl,
    A, S, D, F, G, H, J, K, L,
    Semicolon, SingleQuote, BackTick, LShift, Backslash,
    Z, X, C, V, B, N, M,
    Comma, Period, FowardSlash, RShift, KeypadAsterisk, LAlt, Space, CapsLock,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10,
    NumLock, ScrollLock,
    Keypad7, Keypad8, Keypad9, KeypadMinus, Keypad4, Keypad5, Keypad6, KeypadPlus,
    Keypad1, Keypad2, Keypad3, Keypad0, KeypadPeriod,
    F11 = 0x57, F12,

    // RELEASED:
    EscR = IbmXt::Esc as u8 | KEY_RELEASED,
    OneR, TwoR, ThreeR, FourR, FiveR, SixR, SevenR, EigthR, NineR, ZeroR,
    MinusR, EqualR, BackspaceR, TabR,
    QR, WR, ER, RR, TR, YR, UR, IR, OR, PR,
    OpenBracketR, CloseBracketR, EnterR, LCtrlR,
    AR, SR, DR, FR, GR, HR, JR, KR, LR,
    SemicolonR, SingleQuoteR, BackTickR, LShiftR, BackslashR,
    ZR, XR, CR, VR, BR, NR, MR,
    CommaR, PeriodR, FowardSlashR, RShiftR, KeypadAsteriskR, LAltR, SpaceR, CapsLockR,
    F1R, F2R, F3R, F4R, F5R, F6R, F7R, F8R, F9R, F10R,
    NumLockR, ScrollLockR,
    Keypad7R, Keypad8R, Keypad9R, KeypadMinusR, Keypad4R, Keypad5R, Keypad6R, KeypadPlusR,
    Keypad1R, Keypad2R, Keypad3R, Keypad0R, KeypadPeriodR,
    F11R = IbmXt::F11 as u8 | KEY_RELEASED, F12R,

    ExtendedByte = 0xE0
}
impl IbmXt {
    pub fn to_char(&self) -> Option<&'static str> {
        if *self as u8 > IbmXt::KeypadPeriod as u8 {
            return None;
        }
        KEY_TO_CHAR[*self as usize - 1]
    }
}
impl TryFrom<u8> for IbmXt {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let value_pressed = value & (0xFF ^ KEY_RELEASED);
        if (value_pressed >= IbmXt::Esc as u8 && value_pressed <= IbmXt::KeypadPeriod as u8)
            || value_pressed == IbmXt::F11 as u8 || value_pressed == IbmXt::F12 as u8
            || value == IbmXt::ExtendedByte as u8
        {
            unsafe { Ok(core::mem::transmute(value)) }
        }
        else {
            Err(())
        }
    }
}
