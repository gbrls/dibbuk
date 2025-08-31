- implement dibbuk as: 
  - a rust library to interact with binaries, using:
    - gdb
    - lldb
    - as backends
    - using an IR to abstract debugee data
    - _**the dbg frontend will be an application of the LIB**_
  - a scripting runtime w repl using steel to create UIs, like:
    - helix
    - using steel
    - using a ratatui TUI as part of the repl environment
  - the application will be modeled as:
    - a runtime that listen to dynamic events that change its state, maybe coming from multiple sources, e.g.:
      - gdb events
      - scheme repl hot reloading
      - network events?
      - filesystem events?
    - a library that abstracts debuggers operations in a structure that reflects how it will be used from the scheme, like:

      ```scheme
      (+ 1 2) ; => 3
      (def (a))
      ```

- add labels to calls
- add leged for colours
- add quotes to string telescope

- publish gdb mi library seperately?
  - write tests!!!

- first write a ui to see lighthouse traces
  - then make it interactive with gdb or whatever

- expose IR as a MCP server

- generate PDF reports using typst???


# SEPTEMBER

- WRITE A DSL - using steel!
  - Generate tests both python? (libdebug? pwntools?) and in RUST to fuzz against each other?
  - Tests using crackmes

# 26/08

- use this to attach to the IO started frozen process
  - file-exec-and-symbols, -target-attach PID.
- 👌 implement a simple binary for using stdin / stdout directly from user to gdb
  - maybe add a steel layer to allow scripting directly to IR commands
- 👌 implement a simple recording funcionality for events; and use this for testing

# 28/08

- **!!!a sync blocking io is missing!!!!**.
  - ⚠️

- steel deep dive:
  - rvals defines the type system
    - maybe should use a convertion from rust types <-> SteelHashMap
    - there's also the custom type trait
```rust
  pub trait CustomType {
    fn as_any_ref(&self) -> &dyn Any;
    fn as_any_ref_mut(&mut self) -> &mut dyn Any;
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
    fn inner_type_id(&self) -> TypeId;
    fn display(&self) -> std::result::Result<String, std::fmt::Error> {
        Ok(format!("#<{}>", self.name()))
    }
    fn as_serializable_steelval(&mut self) -> Option<SerializableSteelVal> {
        None
    }
    fn drop_mut(&mut self, _drop_handler: &mut IterativeDropHandler) {}
    fn visit_children(&self, _context: &mut MarkAndSweepContext) {}
    fn visit_children_for_equality(&self, _visitor: &mut cycles::EqualityVisitor) {}
    fn check_equality_hint(&self, _other: &dyn CustomType) -> bool {
        true
    }
    fn check_equality_hint_general(&self, _other: &SteelVal) -> bool {
        false
    }
}
```
- this `inner_type_id` is interesting, maybe it could cause safety issues?


- IDEA 00:
  - translate MI operations directly to scheme
  - current type hierarchy:
    - MiRecord
      - types
        - Result
        - AsyncExec
        - AsyncStatus
        - AsyncNotify
      - fields
        - token
        - class
        - results

    - MiValue 
      - Const
      - Tuple ; more like a map, since it has keys
      - List

    - ConsoleStream
    - TargetStream
    - LogStream
    - GdbPrompt
    - Unknown


- SteelDynamicComponent struct for Steel -> Rust interop
  - https://github.com/mattwparas/helix/blob/dbe0b76d390b0c4f5ba99214bf924b3381d7e30b/helix-term/src/commands/engine/steel/components.rs#L1739

# 29/08

- organize steel integration/scripting into a module
  - write tests for it

- make a UI scripting system


# 30/08

- cor ctf notes
I think the main thing is just localization, i.e., where I'm in the program In the frog challenge for example, the first step was to understand the security protections. By triggering some test cases and seeing the error conditions
After that it was, understanding how the protection was implemented, and where our input was being inserted on the program. After having some ideas about how to bypass those protections I started validate the hipothesis doing a simple POC.

For the `frog` challenge specifically, it wasn't that straight forward to debug the behaviour since the bounds check happened in the execution of a common instruction.

At this point, getting myself localized in the program wasn't a matter of just location ($rip), but also a matter of understanding the stack variables and how they were behaving.


- solution proposal
To make this workflom, more efficient, the gdb scripting environment neatly integrated with the UI / visualization I believe would be a good solution. To basically dynamically and iteratively and interactively build a tool, as you solve the CTF chal itself, with the goal to make it easier than to use the RAW gdb stuff, for an experienced user after SOME learning curve.

See, the conceptual model of the UIs and abstractions is that (at least for me), I get occupied with mapping the $rbp-0x100, $rbp-0x200, $rip+0x1337 and stuff to variables related to the functionality of the program.
ALSO, iterations of gdb runs, something went wrong, I'll change this or that and run it again, but that information is basically lost to gdb apart from the commands history.

I also think that during my vacations I should measure well the time that is spent on dibbuk, it's not ideal to use it to stuff that I can do in the future; right now the work should be focused on BIG & HARD (tm) stuff, like:
  - The repl UI paradigm
  - Tests / validation infrastructure
  - Some automations / stuff that will make easier to keep developing application stuff on top of the infra
  - INFRA
  - Validating lots of ideas an throwing most to the trash

I think this follows a good order of priority.

Also, a TRUE TUI that works well for exploit development is just not much to ask, I don't like text moving all over the place, too many things changing without any semantic meaning. If there's some change in the syntax (ui), after some iteration, it should be caused by some semantic change on debugged program, it's just how my brain is wired, doesn't matter if looking at that bright light is not good for my eyes, it's just too tempting to not keep looking at that shiny, bright and blinking thing; I know quite a few people that relate to this.
