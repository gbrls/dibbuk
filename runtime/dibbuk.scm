; (require "~/")

(define (rato/paragraph text)
  (hash 'Paragraph (hash 'bordered #true 'text text)))

(define (rato/list items)
  (hash 'List items))

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
;

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

(define (dibbuk/make-next state result)
  (cond
    ((dibbuk/state-and-command? result) result)
    ((dibbuk/req? result)
      (list state result))
    (#true
      (list state (dibbuk/none)))))

(define (state-insert k v)
  (list (hash-insert *dibbuk-state* k v) (dibbuk/none)))

(define *dibbuk-command* (dibbuk/none))
(define *dibbuk-state* (hash 'verbose #false))
(define *rato-ui-str* (hash))

(define (rato/tick? r) (TermTick? r))
(define (rato/clear) (TerminalClear))

(define (dibbuk/handle-event state evt-str)
  (let ((result (handle-event state (string->jsexpr evt-str))))
    (set! *dibbuk-state* (first result))
    (set! *dibbuk-command* (second result))
    (if (hash-contains? *dibbuk-state* 'rato-ui)
      (set! *rato-ui-str* (value->jsexpr-string (hash-ref *dibbuk-state* 'rato-ui))))))

(define (dibbuk/hello)
  (set! *dibbuk-command* (dibbuk/cmd (list "a" "b"))))

(define (update-command-history state str)
  (letrec ((state-history (hash-get! state (list 'ui 'command-history)))
           (history (if (list? state-history)
                     (append state-history (list str))
                     (list str))))
    (hash-nested-insert state (list 'ui) (hash 'command-history history))))

(define (eval-user-command state str)
  (if (starts-with? str ":")
    (let ((expr-result (->
                        str
                        (trim-start-matches ":")
                        (eval-string))))
      ; (displayln "=>" expr-result)
      (if (dibbuk/next? expr-result)
        expr-result
        (list
          (-> state
            (update-command-history (string-append "user> " str))
            (update-command-history (string-append "=> " (to-string expr-result))))
          (dibbuk/none))))
    ; just send to gdb instead
    (list
      (update-command-history state (string-append "user> " str))
      (dibbuk/cmd (list str)))))

(define (state-update state key value)
  (letrec ((old-substate (if (hash-contains? state key)
                          (hash-ref state key)
                          (hash)))
           (new-substate (hash-union value old-substate)))
    (hash-insert state key new-substate)))

(define (hash-get! h keys)
  (cond
    ((empty? keys) h)
    ((= 1 (length keys))
      (if (hash-contains? h (first keys))
        (hash-ref h (first keys))
        None))
    (#true
      (if (hash-contains? h (first keys))
        (hash-get! (hash-ref h (first keys)) (cdr keys))
        None))))

(define (hash-nested-insert h keys value)
  (cond
    ((empty? keys) h)
    ((= 1 (length keys))
      (letrec ((k (first keys))
               (old-substate (if (hash-contains? h k)
                              (hash-ref h k)
                              (hash)))
               (new-substate (hash-union value old-substate)))
        (hash-insert h k new-substate)))
    (#true
      (letrec ((k (first keys))
               (old-substate (if (hash-contains? h k)
                              (hash-ref h k)
                              (hash)))
               (new-substate (hash-nested-insert old-substate (cdr keys) value)))
        (hash-insert h k new-substate)))
    ;
    ))

(define (handle-tab state)
  (letrec ((current
             (if (hash-contains? (hash-ref state 'ui) 'current-widget)
               (hash-get! state (list 'ui 'current-widget))
               'history))
           (next
             (cond
               ((symbol=? current 'history) 'registers)
               ((symbol=? current 'registers) 'history))))
    (hash-nested-insert state (list 'ui) (hash 'current-widget next))))

(define (handle-user-key k state #:control [control? #false])
  (letrec ((has-ui (hash-contains? state 'ui))
           (has-buf (and
                     has-ui
                     (hash-contains? (hash-ref state 'ui) 'command-buffer)))
           (command-buf (if has-buf
                         (-> state (hash-ref 'ui) (hash-ref 'command-buffer))
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
               (should-eval? (eval-user-command state command-buf))
               ((and (string=? command-buf "") (string=? key "Return")) (dibbuk/reload))
               ((string=? key "Tab") (list (handle-tab state) (dibbuk/none)))
               (#true (dibbuk/none))))
           (updated-ui-state (hash-nested-insert
                              (if (dibbuk/state-and-command? exec-result)
                                (first exec-result)
                                state)
                              (list 'ui)
                              (hash 'command-buffer command-buf-next)))
           (merged-result (if (dibbuk/state-and-command? exec-result)
                           (list
                             (hash-insert (first exec-result) 'ui (hash-ref updated-ui-state 'ui)) ; move the updated UI state to the exec result state
                             (second exec-result))
                           exec-result)))

    ; (displayln "\r=>" command-buf-next command-buf "\t" k control? should-eval?)
    ; (displayln "\rdibbuk>" command-buf-next)
    (dibbuk/make-next updated-ui-state merged-result)))

(define *debug-registers* (hashset "rax" "rbx" "rdi" "rbp" "rsp" "rdx" "rsi" "rip"))

(define (debug-print-regs state)
  (if (hash-contains? state 'register-state)
    ; (displayln (hash-ref state 'register-state))

    (-> state
      (hash-ref 'register-state)
      (hash-keys->list)
      (transduce (filtering (lambda (name) (hashset-contains? *debug-registers* name))) (into-list))
      (transduce (mapping (lambda (name)
                           (string-append name " " (hash-ref (hash-ref state 'register-state) name))))
        (into-list))
      ;
      )))

(define (ui/registers state)
  (hash-nested-insert
    state
    (list 'ui 'registers)
    (hash 'widget (rato/list (debug-print-regs state)))))

(define (ui/history state)
  (letrec
    ((hist (hash-get! state (list 'ui 'command-history)))
      (hist-list (append
                  (if (list? hist)
                    hist
                    (list "...empty"))
                  (list
                    (string-append "dibbuk> " (hash-get! state (list 'ui 'command-buffer)))))))

    (hash-nested-insert state (list 'ui 'history) (hash 'widget (rato/list hist-list)))))

(define (draw-ui state)
  ; (rato/clear)
  ; (debug-print-regs state)
  ; (displayln "\rHi")
  ;
  (letrec ((regs-str (if (hash-contains? state 'register-state)
                      (debug-print-regs state)
                      "..."))
           (hist (hash-get! state (list 'ui 'command-history)))
           (hist-list (append
                       (if (list? hist)
                         hist
                         (list "...empty"))
                       (list
                         (string-append "dibbuk> " (hash-get! state (list 'ui 'command-buffer))))))
           (current-widget (if (-> state (hash-ref 'ui) (hash-contains? 'current-widget))
                            (-> state (hash-ref 'ui) (hash-ref 'current-widget))
                            'history))
           (updated-state
             (->
               state
               (ui/registers)
               (ui/history))))

    (list (hash-insert
           updated-state
           'rato-ui
           (hash-get! updated-state (list 'ui current-widget 'widget)))
      (dibbuk/none))))

; #hash((Paragraph . '#hash((bordered . #true) (text . "RATATAATATA"))))

; "{\"Paragraph\":{\"text\":\"Hello RATO!\",\"bordered\":true}}"
(define (handle-event state event)
  (let (
        (result
          (event-case event

            ((TerminalUpdate e)
              (if (string=? e "Tick")
                (draw-ui state)))

            ((ConsoleStream s)
              (list
                (update-command-history state s)
                (dibbuk/none)))

            ((LogStream s)
              ; Not sure what the semantics of a LogStream are on GDB MI, but it returns the user input here, so I'm doing nothing with it to not repeat the user input
              ; (displayln (trim-end-matches s "\n"))
              (trim-end-matches s "\n"))

            ((UserInput s)
              (eval-user-command state s))

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
      (->

        state
        (update-command-history "[debug] event:")
        (update-command-history (to-string event))
        (update-command-history "[debug] result:")
        (update-command-history (to-string result))
        (dibbuk/make-next result))

      (dibbuk/make-next state result))

    ; handle return logic, like understanding the format
    ; this is kinda error prone, so it's probably best to write a struct here
    ))

; Get PID
; thread-info
;
; Basic register commands to run on STOPPED
; data-list-changed-registers
; data-list-register-names
; data-list-register-values
;asdf
