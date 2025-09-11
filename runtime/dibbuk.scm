(require "rato.scm")
(require "dibbuk-lib.scm")

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

(define-syntax with-state
  (syntax-rules (else)
    ;; Base case: no bindings
    ((_ state () body (else else-body))
      (if #t body else-body))

    ;; General case
    ((_ state ((var (keys ...)) ...) body (else else-body))
      (if (and (hash-get! state (list keys ...) #:default #false) ...)
        (let ((var (hash-get! state (list keys ...))) ...)
          body)
        else-body))))

(define (hash-get! h keys #:default [default #false])
  (cond
    ((empty? keys) h)
    ((= 1 (length keys))
      (if (hash-contains? h (first keys))
        (hash-ref h (first keys))
        default))
    (#true
      (if (hash-contains? h (first keys))
        (hash-get! (hash-ref h (first keys)) (cdr keys) #:default default)
        default))))

(define (hash-join! h keys value)
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
               (new-substate (hash-join! old-substate (cdr keys) value)))
        (hash-insert h k new-substate)))))

(define (list->sort l)
  (rust.list->sort l))

(define (radix-string->int s base)
  (rust.radix-string->int s base))

(define (int->hex s #:leading [leading 2])
  (rust.int->hex s leading))

(define (addrmaps-filter-map state addr f)
  (if (hash-get! state (list 'target 'maps) #:default #false)
    (transduce
      (hash-get! state (list 'target 'maps))
      (filtering (lambda (mp) (dibbuk/addrmap-contains? mp addr)))
      (mapping f)
      (into-list))
    (list)))

(define (addr? state addr)
  (not (empty? (addrmaps-filter-map state addr (lambda (x) x)))))

(define (addr-perms state addr)
  (let ((res (addrmaps-filter-map state addr (lambda (m) (dibbuk/addrmap-flags m)))))
    (if (not (empty? res))
      (first res)
      "")))

(define (ui/push-history state str)
  (letrec ((history (hash-get! state (list 'ui 'command-history) #:default (list))))
    (-> state
      (hash-join!
        (list 'ui)
        (hash
          'command-history
          (append history (list str))))
      (hash-join! (list 'ui 'history) (hash 'focus (+ 1 (length history)))))))

(define (ui/push-command state name f)
  (hash-join! state (list 'ui 'commands) (hash name f)))

(define *dibbuk-command* (dibbuk/none))
(define *dibbuk-state* (->
                        (hash 'verbose #false)
                        (ui/push-history "===> [welcome to DIBBUK] <===\n~~~ a gdb tui possessed by a lispy spirit ~~~\n\n")
                        (ui/push-command "echo"
                          (lambda (state str) (ui/push-history state (string-join str " "))))

                        (ui/push-command "vmmap"
                          (lambda (state str)
                            (ui/push-history state (to-string
                                                    (if (and (> (length str) 0) (hash-get! state (list 'register-state (first str))))
                                                      (addrmaps-filter-map state (hash-get! state (list 'register-state (first str))) (lambda (x) x))
                                                      (hash-get! state (list 'target 'maps) #:default "Target process not loaded yet!"))))))
                        ;
                        ))
(define *rato-ui-str* (hash))

(define (state-insert k v)
  (list (hash-insert *dibbuk-state* k v) (dibbuk/none)))

(define (dibbuk/handle-event state evt-str)
  (let ((result (handle-event state (string->jsexpr evt-str))))
    (set! *dibbuk-state* (first result))
    (set! *dibbuk-command* (second result))
    (if (hash-contains? *dibbuk-state* 'rato-ui)
      (set! *rato-ui-str* (value->jsexpr-string (hash-ref *dibbuk-state* 'rato-ui))))))

(define (eval-user-command state str)
  (cond
    ; Direct Steel command
    ((starts-with? str ":")
      (let ((expr-result (->
                          str
                          (trim-start-matches ":")
                          (eval-string))))
        ; (displayln "=>" expr-result)
        (if (dibbuk/next? expr-result)
          expr-result
          (list
            (-> state
              (ui/push-history (string-append "user> " str))
              (ui/push-history (string-append
                                "=> "
                                (if (> (string-length (to-string expr-result)) (* 1024 1))
                                  (string-append (substring (to-string expr-result) 0 (* 1024 1)) "\n... (too big)")
                                  (to-string expr-result)))))
            (dibbuk/none)))))

    ((hash-get! state (list 'ui 'commands (first (split-whitespace str))))
      (letrec ((cmd (first (split-whitespace str)))
               (f (hash-get! state (list 'ui 'commands cmd))))
        (list (f (->
                  state
                  (ui/push-history (string-append "user> " str)))
               (rest (split-whitespace str)))
          (dibbuk/none))))

    ; just send to gdb instead
    (#true (list
            (ui/push-history state (string-append "user> " str))
            (dibbuk/cmd (list str))))))

(define (state-update state key value)
  (letrec ((old-substate (if (hash-contains? state key)
                          (hash-ref state key)
                          (hash)))
           (new-substate (hash-union value old-substate)))
    (hash-insert state key new-substate)))

(define (handle-tab state)
  (letrec ((current
             (hash-get! state (list 'ui 'current-widget) #:default 'history))
           (next
             (cond
               ((symbol=? current 'history) 'registers)
               ((symbol=? current 'registers) 'history))))
    (hash-join! state (list 'ui) (hash 'current-widget next))))

(define (handle-ctrl-up state)
  (let ((cur (hash-get! state (list 'ui 'history 'focus) #:default 0)))

    (hash-join! state (list 'ui 'history) (hash 'focus (max 8 (- cur 8))))))

(define (handle-ctrl-down state)
  (let ((cur (hash-get! state (list 'ui 'history 'focus) #:default 0))
        (max-len (length (hash-get! state (list 'ui 'command-history) #:default (list)))))

    (hash-join! state (list 'ui 'history) (hash 'focus (min max-len (+ cur 8))))))

(define (handle-user-key k state #:control [control? #false])
  ; (displayln k)
  (letrec ((has-ui (hash-contains? state 'ui))
           (has-buf (and
                     has-ui
                     (hash-contains? (hash-ref state 'ui) 'command-buffer)))
           (current-command-string (if has-buf
                                    (-> state (hash-ref 'ui) (hash-ref 'command-buffer))
                                    ""))
           (key (first k))
           (modifiers (second k))
           (eval-user-input (and
                             (string=? key "Return")
                             (not (string=? current-command-string ""))))

           (command-buf-next
             (cond
               (eval-user-input "")
               ((and (not control?) (not (> modifiers 1))) (string-append current-command-string key))
               ((string=? key "Backspace") (substring current-command-string 0 (max 0 (- (string-length current-command-string) 1))))
               (#true current-command-string)))

           (exec-result
             (cond
               (eval-user-input (eval-user-command state current-command-string))
               ((and (string=? current-command-string "") (string=? key "Return")) (dibbuk/reload))
               ((string=? key "Tab") (list (handle-tab state) (dibbuk/none)))
               ((and (string=? key "u") (= modifiers 2)) (list (handle-ctrl-up state) (dibbuk/none)))
               ((and (string=? key "d") (= modifiers 2)) (list (handle-ctrl-down state) (dibbuk/none)))
               (#true (dibbuk/none))))

           (next-ui (hash-join!
                     (cond
                       ((begin
                           (dibbuk/state-and-command? exec-result))
                         (first exec-result))
                       (#true
                         state))
                     (list 'ui)
                     (hash 'command-buffer command-buf-next)))

           (next-state (if (dibbuk/state-and-command? exec-result)
                        (list
                          (hash-insert (first exec-result) 'ui (hash-ref next-ui 'ui)) ; move the updated UI state to the exec result state
                          (second exec-result))
                        exec-result)))

    ; (displayln "\r=>" command-buf-next current-command-string "\t" k control? eval-user-input)
    ; (displayln "\rdibbuk>" command-buf-next)
    ;
    ; FIXME: this is very ugly, I should rewrite this
    (dibbuk/make-next next-ui next-state)

    ;
    ))

; (define *debug-registers* (hashset "rip"))
(define *debug-registers* (hashset "rax" "rbx" "rcx" "rdx" "rdi" "rsi" "rbp" "rsp" "rip" "r8" "r9" "r10" "r11" "r12"))

(define (debug-print-regs state)
  (if (hash-contains? state 'register-state)
    ; (displayln (hash-ref state 'register-state))

    (-> state
      (hash-ref 'register-state)
      (hash-keys->list)
      (transduce (filtering (lambda (name) (hashset-contains? *debug-registers* name))) (into-list))
      (transduce (mapping (lambda (name)
                           (string-append
                             name
                             " "
                             (int->hex (hash-get! state (list 'register-state name)) #:leading 16)

                             ;
                             (if
                               (addr? state (hash-get! state (list 'register-state name)))
                               (string-append "   " (addr-perms state (hash-get! state (list 'register-state name))))
                               "")
                             ;
                             )))
        (into-list))
      ;
      )
    (list)))

(define (ui/registers state)
  (hash-join!
    state
    (list 'ui 'registers)
    (hash 'widget (rato/list (debug-print-regs state)))))

(define (disasm-ip-segment state)
  (with-state state ((rip ('register-state "rip"))
                     (pid ('target 'pid)))
    (hash-union
      (->
        state
        (addrmaps-filter-map rip (lambda (x) x))
        (first)
        (dibbuk/disasm-at pid rip))
      ; (hash)

      (->
        state
        (addrmaps-filter-map rip (lambda (x) x))
        (first)
        (dibbuk/disasm-map pid 0)))
    (else (hash))))

(define (show-ip-segment state)
  (with-state state ((rip ('register-state "rip"))
                     (pid ('target 'pid)))
    (->
      state
      (addrmaps-filter-map rip (lambda (x) x))
      (first))
    (else (hash))))

(define (ui/disasm state)
  (with-state state ((rip ('register-state "rip")))
    (letrec ((disasm (disasm-ip-segment state)))
      (hash-join! state (list 'ui 'disasm)
        (hash 'widget (rato/list
                       (->
                         disasm
                         (hash-keys->list)
                         (transduce (mapping (lambda (addr) (- addr rip))) (into-list))
                         ; TODO: FIX: ERROR: on different segments address get mapped wrong for some reason
                         (transduce
                           (filtering
                             (lambda (diff) (and
                                             ; (>= (+ diff 32) 0)
                                             (>= diff 0)
                                             ;
                                             (< diff 64))))
                           (into-list))
                         (list->sort)
                         (transduce (mapping (lambda (off) (string-append
                                                            ; "rip "
                                                            (int->hex (+ rip off) #:leading 16)
                                                            "| "
                                                            (to-string (hash-ref disasm (+ off rip))))))
                           (into-list)))
                       ;
                       ))))
    (else state)))

(define (ui/memory state)
  (hash-join! state
    (list 'ui 'memory)
    (hash 'widget
      (with-state state
        ((pid ('target 'pid))
          (ptr ('register-state "rsp")))
        (->
          (range 32)

          (transduce (mapping (lambda (i) (string-append (int->hex (+ ptr (* i 8)) #:leading 16)
                                           "| "
                                           (->
                                             (dibbuk/read-mem pid (+ ptr (* i 8)) 8)
                                             (transduce (mapping (lambda (byte) (int->hex byte))) (into-list))
                                             (string-join " ")
                                             ;
                                             ))))
            (into-list))
          (rato/list))
        (else (rato/list (list "nothing...")))))))

; TODO: add a title that says 13/37 when the focus is shifted up
(define (ui/history state)
  (letrec
    ((hist (hash-get! state (list 'ui 'command-history) #:default (list "... Empty!")))
      (hist-list (append
                  hist
                  (list
                    (string-append "dibbuk> " (hash-get! state (list 'ui 'command-buffer) #:default ""))))))

    (hash-join! state
      (list 'ui 'history)
      (hash 'widget (rato/list hist-list #:focused (hash-get! state (list 'ui 'history 'focus) #:default (length hist-list)))))))

(define (draw-ui state)
  (letrec ((current-widget (hash-get! state (list 'ui 'current-widget) #:default 'history))
           (updated-state
             (->
               state
               (ui/registers)
               (ui/memory)
               (ui/disasm)
               (ui/history))))

    (list (hash-insert
           updated-state
           'rato-ui
           (rato/ui (list
                     (hash-get! updated-state (list 'ui 'history 'widget) #:default (rato/paragraph "...ops"))
                     (hash-get! updated-state (list 'ui 'registers 'widget) #:default (rato/paragraph "...ops"))
                     (hash-get! updated-state (list 'ui 'disasm 'widget) #:default (rato/paragraph "...ops"))
                     (hash-get! updated-state (list 'ui 'memory 'widget) #:default (rato/paragraph "...ops")))
             (rato/layout
               (list
                 (rato/layout-single)
                 (rato/layout (list
                               (rato/layout-single)
                               (rato/layout-single)
                               (rato/layout-single))
                   'SplitV))
               'SplitH)))
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
                (ui/push-history state (to-string s))
                (dibbuk/none)))

            ((LogStream s)
              ; Not sure what the semantics of a LogStream are on GDB MI, but it returns the user input here, so I'm doing nothing with it to not repeat the user input
              ; (displayln (trim-end-matches s "\n"))
              (list
                (ui/push-history state s)
                (dibbuk/none)))

            ((UserInput s)
              (eval-user-command state s))

            ((Key k)
              (handle-user-key k state))

            ((ControlKey k)
              (handle-user-key k state #:control #true))

            ((NotifyAsync e)
              (if (string=? "thread-group-started" (hash-ref e 'class))
                (dibbuk/cmd (list
                             "-data-list-register-names"
                             "-thread-info"))
                (dibbuk/none)))

            ((ExecAsync e)
              (if (string=? "stopped" (hash-ref e 'class))
                (begin
                  (list
                    (if (hash-get! state (list 'target 'pid) #:default #false)
                      (->
                        state
                        ; maybe add a different place to log debug messages
                        (ui/push-history (string-append
                                          "> PID "
                                          (to-string (hash-get! state (list 'target 'pid)))
                                          " stopped."))
                        (hash-join! (list 'target) (hash 'maps (dibbuk/proc-maps (hash-get! state (list 'target 'pid))))))
                      state)
                    (dibbuk/cmd (list "-data-list-changed-registers"))))
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

                  ; Handle reading the IDs of changes registers
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
                                                 (->
                                                   l
                                                   (second)
                                                   (second)
                                                   (hash-ref 'Const)
                                                   (trim-start-matches "0x")
                                                   (radix-string->int 16)))))
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

                  ((threads procs)
                    (letrec ((meta (->
                                    procs
                                    (hash-ref 'List)
                                    (first)
                                    (hash-ref 'Tuple)))
                             (name (-> meta
                                    (transduce (filtering (lambda (tuple) (string=? (first tuple) "name"))) (into-list))
                                    (first)
                                    (second)
                                    (hash-ref 'Const)))

                             (pid (-> meta
                                   (transduce (filtering (lambda (tuple) (string=? (first tuple) "target-id"))) (into-list))
                                   (first)
                                   (second)
                                   (hash-ref 'Const)
                                   (split-whitespace)
                                   (second)
                                   (string->int)))
                             ; (transduce (mapping (lambda (name)
                             ; (string-append name " " (hash-ref (hash-ref state 'register-state) name))))
                             ; (into-list)))
                             ;
                             )
                      (list (->
                             state
                             (ui/push-history (string-append "THREADS> " (to-string id)))
                             (hash-join! (list 'target) (hash 'meta meta 'name name 'pid pid))
                             (hash-join! (list 'target) (hash 'maps (dibbuk/proc-maps pid))))
                        (dibbuk/none))))

                  (else result))))
            (else #false))))

    (if
      (hash-get! state (list 'verbose))
      ; (not
      ;   (and
      ;     (hash? event)
      ;     (string=? "Tick" (hash-get! event (list 'TerminalUpdate) #:default ""))))
      ;
      (->
        state
        (ui/push-history "[debug] event:")
        (ui/push-history (to-string event))
        (ui/push-history "[debug] result:")
        (ui/push-history (to-string result))
        (dibbuk/make-next result))

      (dibbuk/make-next state result))))

; Get PID
; thread-info
;
; Basic register commands to run on STOPPED
; data-list-changed-registers
; data-list-register-names
; data-list-register-values
;asdf
