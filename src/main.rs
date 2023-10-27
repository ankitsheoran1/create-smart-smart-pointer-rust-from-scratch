fn main() {
    println!("Hello, world!");
}


use std::cell::UnsafeCell;
use crate::cell::Cell;

pub struct Cell<T> {
    value: UnsafeCell<T>,
}

// implied by UnsafeCell
// impl<T> !Sync for Cell<T> {}

impl<T> Cell<T> {
    pub fn new(value: T) -> Self {
        Cell {
            value: UnsafeCell::new(value),
        }
    }

    pub fn set(&self, value: T) {
        // SAFETY: we know no-one else is concurrently mutating self.value (because !Sync)
        // SAFETY: we know we're not invalidating any references, because we never give any out
        unsafe { *self.value.get() = value };
    }

    pub fn get(&self) -> T
    where
        T: Copy,
    {
        // SAFETY: we know no-one else is modifying this value, since only this thread can mutate
        // (because !Sync), and it is executing this function instead.
        unsafe { *self.value.get() }
    }
}

#[derive(Copy, Clone)]
enum RefState {
    Unshared,
    Shared(usize),
    Excusive,
}

pub struct RefCell<T> {
    value: UnsafeCell<T>,
    // to make thread safe we can make this a cell 
    state: Cell<RefState>,
}

impl<T> RefCell<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value)
            state: Cell::New(RefState::Unshared),
        }
    }

    pub fn borrow_mut(&self) -> Option<RefMut<'_, T>> {
        if let RefState::Unshared == self.state.get() {
            self.state.set(RefState::Excusive);
            Some(RefMut { refcell: self })
        } else {
            None
        }
    }

    pub fn borrow(&self) -> Option<Ref<'_, T>>  {
        match self.state.get() {
            RefState::Unshared => {
                self.state.set(RefState::Shared(1));
                Some(Ref { refcell: self }),
            }
            RefState::Shared(n) => {
                self.state.set(RefState::Shared(n + 1));
                Some(Ref { refcell: self }),
            }
            RefState::Excusive => None 
        }
    }
}

// This is trait we used to maintain borrow checker at runtime rather then compile time as we have used unsafe code 
pub struct Ref<'refcell, T> {
    refcell: &'refcell RefCell<T>,
}

impl<T> Deref for Ref<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        // SAFETY
        // a Ref is only created if no exclusive references have been given out.
        // once it is given out, state is set to Shared, so no exclusive references are given out.
        // so dereferencing into a shared reference is fine.
        unsafe {&*self.Target.value.get()}
    }
}

impl<T> Drop for Ref<'_, T> {
    fn drop(&self) {
        match self.refcell.state.get() {
            RefState::Exclusive | RefState::Unshared => unreachable!(),
            RefState::Shared(1) => {
                self.refcell.state.set(RefState::Unshared);
            }
            RefState::Shared(n) => {
                self.refcell.state.set(RefState::Shared(n - 1));
            }
        }
    }

}

pub struct RefMut<'refcell, T> {
    refcell: &'refcell RefCell<T>,
}

impl<T> Deref for RefMut<'refcell, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        // SAFETY
        // see safety for DerefMut
        unsafe {&*self.refcell.value.get()}
    }
}

impl<T> std::ops::DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY
        // a RefMut is only created if no other references have been given out.
        // once it is given out, state is set to Exclusive, so no future references are given out.
        // so we have an exclusive lease on the inner value, so mutably dereferencing is fine.
        unsafe { &mut *self.refcell.value.get() }
    }
}

impl<T> Drop for RefMut<'_, T> {
    fn drop(&mut self) {
        match self.refcell.state.get() {
            RefState::Shared(_) | RefState::Unshared => unreachable!(),
            RefState::Exclusive => {
                self.refcell.state.set(RefState::Unshared);
            }
        }
    }
}

// RefCell - its safe dynamically check borrowing 

// to tell compiler u can never share across threads 
// Unsafe cell is also imolements this so we can get this already 
// impl<T> !Sync for Cell<T> {}


#[cfg(test)]
mod test {

    #[test]
fn concurrent_set() {
    use std::sync::Arc;
    let x = Arc::new(Cell::new(42));
    let x1 = Arc::clone(&x);
    std::thread::spawn(move || {
        x1.set(43);
    });
    let x2 = Arc::clone(&x);
    std::thread::spawn(move || {
        x2.set(44);
    });
}

#[test]
fn set_during_get() {
    let x = Cell::new(String::from("hello"));
    let first = x.get();
    x.set(String::new());
    x.set(String::from("world"));
    eprintln!("{}", first);
}

#[test]
fn concurrent_set_take2() {
    use std::sync::Arc;
    let x = Arc::new(Cell::new([0; 40240]));
    let x1 = Arc::clone(&x);
    let jh1 = std::thread::spawn(move || {
        x1.set([1; 40240]);
    });
    let x2 = Arc::clone(&x);
    let jh2 = std::thread::spawn(move || {
        x2.set([2; 40240]);
    });
    jh1.join().unwrap();
    jh2.join().unwrap();
    let xs = x.get();
    for &i in xs.iter() {
        eprintln!("{}", i);
    }
}

#[test]
fn concurrent_get_set() {
    use std::sync::Arc;
    let x = Arc::new(Cell::new(0));
    let x1 = Arc::clone(&x);
    let jh1 = std::thread::spawn(move || {
        for _ in 0..1000000 {
            let x = x1.get();
            x1.set(x + 1);
        }
    });
    let x2 = Arc::clone(&x);
    let jh2 = std::thread::spawn(move || {
        for _ in 0..1000000 {
            let x = x2.get();
            x2.set(x + 1);
        }
    });
    jh1.join().unwrap();
    jh2.join().unwrap();
    assert_eq!(x.get(), 2000000);
}

}