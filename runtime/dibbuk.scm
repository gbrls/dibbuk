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
(define (dibbuk/state-and-command? result)
  (and
    (list? result)
    (hash? (first result))
    (dibbuk/req? (second result))))
(define (dibbuk/next? result)
  (or (dibbuk/state-and-command? result)
    (dibbuk/req? result)))

(define (dibbuk/make-next result state)
  (cond
    ((dibbuk/state-and-command? result) result)
    ((dibbuk/req? result)
      (list state result))
    (#true
      (list state (dibbuk/none)))))

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
      (if (dibbuk/next? expr-result) expr-result (dibbuk/none)))
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
                          (not (string=? command-buf ""))))

           (command-buf-next
             (cond
               (should-eval? "")
               ((not control?) (string-append command-buf key))
               ((string=? key "Backspace") (substring command-buf 0 (max 0 (- (string-length command-buf) 1))))
               (#true command-buf)))

           (exec-result
             (cond
               (should-eval? (eval-user-command command-buf))
               ((and (string=? command-buf "") (string=? key "Return")) (dibbuk/reload))
               (#true (dibbuk/none))))
           (updated-ui-state (state-update
                              state
                              'ui
                              (hash *dibbuk-ui-command-buffer* command-buf-next)))
           (merged-result (if (dibbuk/state-and-command? exec-result)
                           (list
                             (hash-insert (first exec-result) 'ui (hash-ref updated-ui-state 'ui)) ; move the updated UI state to the exec result state
                             (second exec-result))
                           exec-result)))

    ; (displayln "\r=>" command-buf-next command-buf "\t" k control? should-eval?)
    (displayln "\rdibbuk>" command-buf-next)
    (dibbuk/make-next merged-result updated-ui-state)))

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

(define (draw-ui state)
  ; (rato/clear)
  ; (debug-print-regs state)
  ; (displayln "\rHi")
  ;
  None)

(define (handle-event state event)
  (let (
        (result
          (event-case event

            ((TerminalUpdate e)
              (if (string=? e "Tick")
                (draw-ui state)))

            ((ConsoleStream s)
              (displayln "\r" (trim-end-matches s "\n")))

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
         (not (and (hash? event) (hash-contains? event 'TerminalUpdate) (string=? "Tick" (hash-ref event 'TerminalUpdate)))))
      (begin

        (displayln "\rinput event:")
        (displayln event)
        (displayln "\rresult:")
        (displayln result)
        (displayln "\r====\n")))

    ; handle return logic, like understanding the format
    ; this is kinda error prone, so it's probably best to write a struct here
    (dibbuk/make-next result state)))

; Get PID
; thread-info
;
; Basic register commands to run on STOPPED
; data-list-changed-registers
; data-list-register-names
; data-list-register-values
;asdf
