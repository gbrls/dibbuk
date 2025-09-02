(define-syntax event-case
  (syntax-rules ()
    ((_ event
        ((key var) body ...)
        ...
        (else else-body ...))
      (cond
        ((-> event hash? not) #false)
        ((hash-contains? event 'key) (let ((var (hash-ref event 'key))) body ...))
        ...
        (#true else-body ...)))))

; ===============
; Aliases for Rust-defined  functions
; ===============

(define (dibbuk/cmd l) (GdbCommandsReq l))
(define (dibbuk/none) (EmptyReq))
(define (dibbuk/reload) (Reload))
(define (dibbuk/req? r) (Request? r))
(define (rato/tick? r) (TermTick? r))
(define (rato/clear) (TerminalClear))

(define *dibbuk-command* (dibbuk/none))
(define *dibbuk-state* (hash 'verbose #false))
(define *dibbuk-ui-command-buffer* 'command-buffer)

(define (state-insert k v)
  (list (hash-insert *dibbuk-state* k v) (dibbuk/none)))

(define (dibbuk/handle-event state evt-str)
  (let ((result (handle-event state (string->jsexpr evt-str))))
    (set! *dibbuk-state* (first result))
    (set! *dibbuk-command* (second result))))

(define (dibbuk/hello)
  (set! *dibbuk-command* (dibbuk/cmd (list "a" "b"))))

(define (eval-user-command str)
  (if (starts-with? str ":")

    (let ((expr-result (->
                        str
                        (trim-start-matches ":")
                        (eval-string))))
      (displayln "=>" expr-result)
      expr-result)
    ; just send to gdb instead
    (dibbuk/cmd (list str))))

(define (state-update state key value)
  (letrec ((old-substate (if (hash-contains? state key)
                          (hash-ref state key)
                          (hash)))
           (new-substate (hash-union value old-substate)))
    (hash-insert state key new-substate)))

(define (handle-user-key k state #:control [control? #false])
  (letrec ((has-ui (hash-contains? state 'ui))
           (has-buf (and
                     has-ui
                     (hash-contains? (hash-ref state 'ui) *dibbuk-ui-command-buffer*)))
           (command-buf (if has-buf
                         (-> state (hash-ref 'ui) (hash-ref *dibbuk-ui-command-buffer*))
                         ""))
           (key (first k))
           (modifiers (second k))
           (should-eval? (and (string=? key "Return")
                          (starts-with? command-buf ":")))

           (runtime-command
             (cond
               ((and (string=? command-buf "") (string=? key "Return")) (dibbuk/reload))
               (should-eval? (eval-user-command command-buf))
               (#true (dibbuk/none))))

           (command-buf-next
             (cond
               ((not control?) (string-append command-buf key))
               ((string=? key "Backspace") (substring command-buf 0 (max 0 (- (string-length command-buf) 1))))
               (should-eval? "")
               (#true command-buf))))

    (displayln "\r=>" command-buf-next "\t" k control? should-eval?)
    (list
      (state-update state 'ui (hash *dibbuk-ui-command-buffer* command-buf-next))
      runtime-command)))

(define *debug-registers* (hashset "rax" "rbx" "rdi" "rbp" "rsp" "rdx" "rsi" "rip"))

(define (debug-print-regs state)
  (if (hash-contains? state 'register-state)
    ; (displayln (hash-ref state 'register-state))

    (displayln
      (-> state
        (hash-ref 'register-state)
        (hash-keys->list)
        (transduce (filtering (lambda (name) (hashset-contains? *debug-registers* name))) (into-list))
        (transduce (mapping (lambda (name)
                             (list name (hash-ref (hash-ref state 'register-state) name))))
          (into-list))
        ;
        ))))

(define (handle-event state event)
  (let (
        (result
          (event-case event

            ((ConsoleStream s)
              (displayln (trim-end-matches s "\n")))

            ((LogStream s)
              ; Not sure what the semantics of a LogStream are on GDB MI, but it returns the user input here, so I'm doing nothing with it to not repeat the user input
              ; (displayln (trim-end-matches s "\n"))
              (trim-end-matches s "\n"))

            ((UserInput s)
              (eval-user-command s))

            ((Key k)
              (handle-user-key k state))
            ((ControlKey k)
              (handle-user-key k state #:control #true))

            ((NotifyAsync e)
              (if (string=? "thread-group-started" (hash-ref e 'class))
                (dibbuk/cmd (list "-data-list-register-names"))
                (dibbuk/none)))

            ((ExecAsync e)
              (if (string=? "stopped" (hash-ref e 'class))
                (begin
                  (dibbuk/cmd (list "-data-list-changed-registers")))
                (dibbuk/none)))

            ((StatusAsync e)
              (hash-ref 'class e))

            ((Result r)
              (let ((result (hash-ref r 'results)))
                (event-case result

                  ((register-names regs)
                    (let ((pairs
                            (->
                              (hash-ref regs 'List)
                              (transduce (mapping (lambda (mp) (hash-ref mp 'Const))) (into-list))
                              (transduce (enumerating) (into-list)))))
                      (list
                        (hash-insert *dibbuk-state* 'register-ids
                          (fold (lambda (x acc) (hash-insert acc (first x) (second x))) (hash) pairs))
                        (dibbuk/none))))

                  ((changed-registers regs)
                    (letrec ((ids
                               (->
                                 (hash-ref regs 'List)
                                 (transduce (mapping (lambda (mp) (hash-ref mp 'Const))) (into-list))
                                 (string-join " ")))
                             (cmd-str (string-append "-data-list-register-values x " ids)))
                      (begin
                        ; (displayln cmd-str)
                        (dibbuk/cmd (list cmd-str)))))

                  ((register-values regs)
                    (letrec ((pairs
                               (->
                                 (hash-ref regs 'List)
                                 (transduce (mapping (lambda (mp) (hash-ref mp 'Tuple))) (into-list))
                                 (transduce (mapping
                                             (lambda (l)
                                               (list
                                                 (-> l (first) (second) (hash-ref 'Const) (string->int))
                                                 (-> l (second) (second) (hash-ref 'Const)))))
                                   (into-list))))
                             (changed
                               (fold (lambda (x acc)
                                      (hash-insert
                                        acc
                                        (-> state (hash-ref 'register-ids) (hash-ref (first x)))
                                        (second x)))
                                 (hash)
                                 pairs)))
                      (list
                        (hash-insert state 'register-state
                          (hash-union changed
                            (if (hash-contains? state 'register-state)
                              (hash-ref state 'register-state)
                              (hash))))
                        (dibbuk/none))))

                  (else result))))
            (else #false))))

    ; verbose debug prints
    (if (and
         (hash-contains? *dibbuk-state* 'verbose)
         (hash-ref *dibbuk-state* 'verbose)
         (hash? event))
      (begin

        (displayln "\rinput event:")
        (displayln event)
        (displayln "\rresult:")
        (displayln result)
        (displayln "\r====\n")))

    ; handle return logic, like understanding the format
    ; this is kinda error prone, so it's probably best to write a struct here
    (cond
      ((and
          (list? result)
          (hash? (first result))
          (dibbuk/req? (second result)))
        result)
      ((dibbuk/req? result)
        (list state result))
      (#true
        (list state (dibbuk/none))))))

; Get PID
; thread-info
;
; Basic register commands to run on STOPPED
; data-list-changed-registers
; data-list-register-names
; data-list-register-values
;asdf
