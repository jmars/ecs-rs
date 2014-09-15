# ECS
## Entity Component System

__ECS__ is an implementation of the ECS pattern in rust, it uses green threads and schedulers to provide
event driven __systems__ for operating over a shared memory data structure containing __entities__ implementing certain
__components__.

# Example
```Rust
extern crate ecs;

use self::ecs::{Entity, App, Component, System, Entities};

#[deriving(Clone)]
enum Event {
    Nothing
}

struct TestComponent {
    stuff: i64
}

impl Component for TestComponent;

struct TestSystem {
    val: String
}

impl System<Event> for TestSystem {
    fn run(mut self, app: Sender<Event>, mut events: Messages<Event>, entities: Entities) {
        for event in events {
            let foo = &self.val;
            match event {
                _ => println!("I has a {}", foo)
            }
        }
    }
}

fn main() {
    let foo = TestSystem { val: "foo".to_string() };
    let systems = vec!(box foo as Box<System<Event> + Send>);
    let test = {
        let mut ent = Entity::new();
        ent.insert(TestComponent { stuff: 100i64 });
        ent
    }
    let entities = vec!(RWLock::new(test));
    let mut app: App<Event> = App::new(Arc::new(entities));
    app.start(systems);
    // ... stuff
    app.shutdown();
}
```
