use anyhow::{anyhow, Context};
use windows::core::{BSTR, GUID, HSTRING, PCWSTR, VARIANT};
use windows::Win32::System::Com::{
    CLSIDFromProgID, CoCreateInstance, CoInitialize, CoUninitialize, IDispatch,
    CLSCTX_LOCAL_SERVER, DISPATCH_FLAGS, DISPATCH_METHOD, DISPATCH_PROPERTYGET,
    DISPATCH_PROPERTYPUT, DISPPARAMS,
};
use windows::Win32::System::Ole::DISPID_PROPERTYPUT;

const LOCALE_USER_DEFAULT: u32 = 0x0400;
const LOCALE_SYSTEM_DEFAULT: u32 = 0x0800;

pub struct Variant(VARIANT);

impl From<bool> for Variant {
    fn from(value: bool) -> Self {
        Self(value.into())
    }
}

impl From<i32> for Variant {
    fn from(value: i32) -> Self {
        Self(value.into())
    }
}

impl From<&str> for Variant {
    fn from(value: &str) -> Self {
        Self(BSTR::from(value).into())
    }
}

impl From<&String> for Variant {
    fn from(value: &String) -> Self {
        Self(BSTR::from(value).into())
    }
}

#[allow(unused)]
impl Variant {
    pub fn bool(&self) -> anyhow::Result<bool> {
        Ok(bool::try_from(&self.0)?)
    }

    pub fn int(&self) -> anyhow::Result<i32> {
        Ok(i32::try_from(&self.0)?)
    }

    pub fn string(&self) -> anyhow::Result<String> {
        Ok(BSTR::try_from(&self.0)?.to_string())
    }

    pub fn idispatch(&self) -> anyhow::Result<IDispatchWrapper> {
        Ok(IDispatchWrapper(IDispatch::try_from(&self.0)?))
    }

    pub fn vt(&self) -> u16 {
        unsafe { self.0.as_raw().Anonymous.Anonymous.vt }
    }
}

pub fn initialize_com() -> anyhow::Result<String> {
    unsafe {
        let res = CoInitialize(None);
        if res.is_err() {
            return Err(anyhow!("error: {}", res.message()));
        }
        Ok(String::from("Ok"))
    }
}

pub struct IDispatchWrapper(pub IDispatch);

#[allow(unused)]
impl IDispatchWrapper {
    pub fn new(name: &String) -> anyhow::Result<Self> {
        unsafe {
            let clsid = CLSIDFromProgID(PCWSTR::from_raw(HSTRING::from(name).as_ptr()))
                .with_context(|| "CLSIDFromProgID")?;

            let app = CoCreateInstance(&clsid, None, CLSCTX_LOCAL_SERVER)
                .with_context(|| "CoCreateInstance")?;

            Ok(IDispatchWrapper(app))
        }
    }

    pub fn invoke(
        &self,
        flags: DISPATCH_FLAGS,
        name: &str,
        mut args: Vec<Variant>,
    ) -> anyhow::Result<Variant> {
        unsafe {
            let mut dispid = 0;
            self.0
                .GetIDsOfNames(
                    &GUID::default(),
                    &PCWSTR::from_raw(HSTRING::from(name).as_ptr()),
                    1,
                    LOCALE_USER_DEFAULT,
                    &mut dispid,
                )
                .with_context(|| "GetIDsOfNames")?;

            let mut dp = DISPPARAMS::default();
            let mut dispid_named = DISPID_PROPERTYPUT;

            if !args.is_empty() {
                args.reverse();
                dp.cArgs = args.len() as u32;
                dp.rgvarg = args.as_mut_ptr() as *mut VARIANT;

                // Handle special-case for property-puts!
                if (flags & DISPATCH_PROPERTYPUT) != DISPATCH_FLAGS(0) {
                    dp.cNamedArgs = 1;
                    dp.rgdispidNamedArgs = &mut dispid_named;
                }
            }

            let mut result = VARIANT::default();
            self.0
                .Invoke(
                    dispid,
                    &GUID::default(),
                    LOCALE_SYSTEM_DEFAULT,
                    flags,
                    &dp,
                    Some(&mut result),
                    None,
                    None,
                )
                .with_context(|| "Invoke")?;

            Ok(Variant(result))
        }
    }

    pub fn get(&self, name: &str) -> anyhow::Result<Variant> {
        self.invoke(DISPATCH_PROPERTYGET, name, vec![])
    }

    pub fn int(&self, name: &str) -> anyhow::Result<i32> {
        let result = self.get(name)?;
        result.int()
    }

    pub fn bool(&self, name: &str) -> anyhow::Result<bool> {
        let result = self.get(name)?;
        result.bool()
    }

    pub fn string(&self, name: &str) -> anyhow::Result<String> {
        let result = self.get(name)?;
        result.string()
    }

    pub fn put(&self, name: &str, args: Vec<Variant>) -> anyhow::Result<Variant> {
        self.invoke(DISPATCH_PROPERTYPUT, name, args)
    }

    pub fn call(&self, name: &str, args: Vec<Variant>) -> anyhow::Result<Variant> {
        self.invoke(DISPATCH_METHOD, name, args)
    }
}

pub struct DeferQuit<'a>(pub &'a IDispatchWrapper);

impl Drop for DeferQuit<'_> {
    fn drop(&mut self) {
        self.0.call("Quit", vec![]).unwrap(); //todo: quit doesn't kill process after open and Visible=true
    }
}

pub struct DeferUninitializeCOM;

impl Drop for DeferUninitializeCOM {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
