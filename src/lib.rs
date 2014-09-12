#![feature(default_type_params)]
#![feature(unboxed_closures)]
#![feature(unboxed_closure_sugar)]
#![feature(overloaded_calls)]

extern crate "unsafe-any" as uany;
extern crate green;

use std::any::Any;
use std::intrinsics::TypeId;
use std::collections::{Collection, HashMap, Mutable};
use std::hash::{Hash, Hasher, Writer};
use std::mem::{transmute};
use std::ptr::copy_nonoverlapping_memory;
use std::sync::{Arc, RWLock};
use std::task::TaskBuilder;
use std::comm::Messages;
use green::{SchedPool, PoolConfig, GreenTaskBuilder};

use self::uany::{UncheckedAnyDowncast, UncheckedAnyMutDowncast};

struct TypeIdHasher;

struct TypeIdState {
    value: u64,
}

pub trait Component: Any + Send {}

impl<'a> Clone for Box<Component + 'a> {
    fn clone(&self) -> Box<Component + 'a> { self.clone() }
}

impl Writer for TypeIdState {
    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) {
        debug_assert!(bytes.len() == 8);
        unsafe {
            copy_nonoverlapping_memory(&mut self.value,
                                                 transmute(&bytes[0]),
                                                 1)
        }
    }
}

impl Hasher<TypeIdState> for TypeIdHasher {
    fn hash<T>(&self, value: &T) -> u64
        where T: Hash<TypeIdState> {
        let mut state = TypeIdState {
            value: 0,
        };
        value.hash(&mut state);
        state.value
    }
}

impl<'a> UncheckedAnyDowncast<'a> for &'a Component+'a {
    #[inline]
    unsafe fn downcast_ref_unchecked<T>(self) -> &'a T where T: 'static { self.downcast_ref_unchecked() }
}

impl<'a> UncheckedAnyMutDowncast<'a> for &'a mut Component+'a {
    #[inline]
    unsafe fn downcast_mut_unchecked<T>(self) -> &'a mut T where T: 'static { self.downcast_mut_unchecked() }
}

pub struct Entity {
    data: HashMap<TypeId, Box<Component + 'static + Send>, TypeIdHasher>,
}

impl Entity {
    pub fn new() -> Entity {
        Entity {
            data: HashMap::with_hasher(TypeIdHasher),
        }
    }

    pub fn find<'a, T>(&'a self) -> Option<&'a T> where T: Component + Send {
        self.data.find(&TypeId::of::<T>()).map(|any| unsafe { any.downcast_ref_unchecked::<T>() })
    }

    pub fn find_mut<'a, T>(&'a mut self) -> Option<&'a mut T> where T: Component + Send {
        self.data.find_mut(&TypeId::of::<T>()).map(|any| unsafe { any.downcast_mut_unchecked::<T>() })
    }

    pub fn insert<T>(&mut self, value: T) where T: Component + Send {
        self.data.insert(TypeId::of::<T>(), box value as Box<Component + Send>);
    }

    pub fn remove<T>(&mut self) where T: Component + Send {
        self.data.remove(&TypeId::of::<T>());
    }

    pub fn contains<T>(&self) -> bool where T: Component + Send {
        self.data.contains_key(&TypeId::of::<T>())
    }
}

impl Collection for Entity {
    fn len(&self) -> uint {
        self.data.len()
    }

    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl Mutable for Entity {
    fn clear(&mut self) {
        self.data.clear();
    }
}

pub type Entities = Arc<Vec<RWLock<Entity>>>;

pub trait System<T: Send> {
    fn run(mut self, app: Sender<T>, events: Messages<T>, entities: Entities);
}

impl<'a, T: Send> FnOnce<(Sender<T>, Messages<'a, T>, Entities), ()> for Box<System<T> + 'static> {
    #[rust_call_abi_hack]
    fn call_once(self, args: (Sender<T>, Messages<T>, Entities)) {
        let (tx, rx, ent) = args;
        self.run(tx, rx, ent)
    }
}

pub type Systems<T> = Vec<Box<System<T> + Send>>;

pub struct App<'a, T> {
    entities: Entities,
    pool: SchedPool,
    events: ((Sender<T>, Receiver<T>), Vec<Sender<T>>)
}

impl<'a, T: Send + Clone> App<'a, T> {
    pub fn new(entities: Entities) -> App<'a, T> {
        let (tx, rx) = channel::<T>();
        let listeners = Vec::new();
        let config = PoolConfig::new();
        let pool = SchedPool::new(config);
        App {
            entities: entities,
            pool: pool,
            events: ((tx, rx), listeners)
        }
    }

    pub fn start(&mut self, systems: Systems<T>) {
        for system in systems.move_iter() {
            let (itx, irx) = channel();
            let tx = self.events.ref0().ref0().clone();
            let entities = self.entities.clone();
            TaskBuilder::new().green(&mut self.pool).spawn(proc() { system(tx, irx.iter(), entities) });
            self.events.mut1().push(itx);
        }
    }

    pub fn shutdown(mut self) {
        self.events.mut1().clear();
        self.pool.shutdown();
    }

    pub fn send(&self, event: T) {
        for listener in self.events.ref1().iter() {
            listener.send(event.clone())
        }
    }

    pub fn recv(&self) -> T {
        self.events.ref0().ref1().recv()
    }
}
