use std::{ fmt, mem, ptr };
use std::ops::{ Deref, DerefMut };
use std::cell::Cell;
use memsec::{ memzero, malloc, free, mprotect, Prot };


/// Secure Key.
///
/// The use [memsec/malloc](../../memsec/fn.malloc.html) protection secret bytes.
/// When you need the password stored in the memory, you should use it.
///
/// More docs see [Secure memory · libsodium](https://download.libsodium.org/doc/helpers/memory_management.html).
pub struct SecKey<T> {
    ptr: *mut T,
    count: Cell<usize>
}

impl<T> Default for SecKey<T> where T: Default {
    fn default() -> Self {
        SecKey::new(T::default())
                .unwrap_or_else(|_| panic!("memsec::malloc fail: {}", mem::size_of::<T>()))
    }
}

impl<T> SecKey<T> where T: Sized {
    /// ```
    /// use seckey::SecKey;
    ///
    /// let k = SecKey::new([1]).unwrap();
    /// assert_eq!([1], *k.read());
    /// ```
    pub fn new(mut t: T) -> Result<SecKey<T>, T> {
        unsafe {
            match Self::from_raw(&t) {
                Some(output) => {
                    memzero(&mut t, mem::size_of::<T>());
                    mem::forget(t);
                    Ok(output)
                },
                None => Err(t)
            }
        }
    }

    /// ```
    /// use seckey::SecKey;
    ///
    /// let mut v = [1];
    /// let k = unsafe { SecKey::from_raw(&v).unwrap() };
    /// assert_eq!([1], v);
    /// assert_eq!([1], *k.read());
    /// ```
    pub unsafe fn from_raw(t: *const T) -> Option<SecKey<T>> {
        let memptr: *mut T = match malloc(mem::size_of::<T>()) {
            Some(memptr) => memptr,
            None => return None
        };
        ptr::copy_nonoverlapping(t, memptr, 1);
        mprotect(memptr, Prot::NoAccess);

        Some(SecKey {
            ptr: memptr,
            count: Cell::new(0)
        })
    }
}

impl<T> SecKey<T> {
    fn read_unlock(&self) {
        let count = self.count.get();
        self.count.set(count + 1);
        if count == 0 {
            unsafe { mprotect(self.ptr, Prot::ReadOnly) };
        }
    }

    fn write_unlock(&self) {
        let count = self.count.get();
        self.count.set(count + 1);
        if count == 0 {
            unsafe { mprotect(self.ptr, Prot::ReadWrite) };
        }
    }

    fn lock(&self) {
        let count = self.count.get();
        self.count.set(count - 1);
        if count <= 1 {
            unsafe { mprotect(self.ptr, Prot::NoAccess) };
        }
    }

    /// Borrow Read.
    ///
    /// ```
    /// use seckey::SecKey;
    ///
    /// let secpass = SecKey::new([8u8; 8]).unwrap();
    /// assert_eq!([8u8; 8], *secpass.read());
    /// ```
    #[inline]
    pub fn read(&self) -> SecReadGuard<T> {
        self.read_unlock();
        SecReadGuard(self)
    }

    /// Borrow Write.
    ///
    /// ```
    /// # use seckey::SecKey;
    /// #
    /// # let mut secpass = SecKey::new([8u8; 8]).unwrap();
    /// let mut wpass = secpass.write();
    /// wpass[0] = 0;
    /// assert_eq!([0, 8, 8, 8, 8, 8, 8, 8], *wpass);
    /// ```
    #[inline]
    pub fn write(&mut self) -> SecWriteGuard<T> {
        self.write_unlock();
        SecWriteGuard(self)
    }
}

impl<T> fmt::Debug for SecKey<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "** sec key ({}) **", self.count.get())
    }
}

impl<T> fmt::Pointer for SecKey<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:p}", self.ptr)
    }
}

impl<T> Drop for SecKey<T> {
    fn drop(&mut self) {
        unsafe {
            mprotect(self.ptr, Prot::ReadWrite);
            ptr::drop_in_place(self.ptr);
            free(self.ptr);
        }
    }
}


/// Read Guard.
pub struct SecReadGuard<'a, T: 'a>(&'a SecKey<T>);

impl<'a, T: 'a> Deref for SecReadGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.0.ptr }
    }
}

impl<'a, T: 'a> Drop for SecReadGuard<'a, T> {
    fn drop(&mut self) {
        self.0.lock();
    }
}


/// Write Guard.
pub struct SecWriteGuard<'a, T: 'a>(&'a mut SecKey<T>);

impl<'a, T: 'a> Deref for SecWriteGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.0.ptr }
    }
}

impl<'a, T: 'a> DerefMut for SecWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.0.ptr }
    }
}

impl<'a, T: 'a> Drop for SecWriteGuard<'a, T> {
    fn drop(&mut self) {
        self.0.lock();
    }
}
