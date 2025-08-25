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


# SEPTEMBER

- WRITE A DSL - using steel!
  - Generate tests both python? (libdebug? pwntools?) and in RUST to fuzz against each other?
  - Tests using crackmes
