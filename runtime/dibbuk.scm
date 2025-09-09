(define (rato/layout children mode)
  (hash 'children children 'mode mode))

(define (rato/layout-single)
  (rato/layout (list) 'Single))

(define (rato/layout-horizontal-2)
  (rato/layout
    (list
      (rato/layout-single)
      (rato/layout-single))
    'SplitH))

(define (rato/layout-vertical-2)
  (rato/layout
    (list
      (rato/layout-single)
      (rato/layout-single))
    'SplitV))

(define (rato/ui widgets layout)
  (hash 'widgets widgets 'layout layout))

(define (rato/paragraph text)
  (hash 'Paragraph (hash 'bordered #true 'text text)))

(define (rato/list items #:focused [focused 0] #:bordered [bordered #true])
  (hash 'List (list items focused bordered)))

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

(define (dibbuk/addrmap-contains? mp addr)
  (MapRange->contains? mp addr))
(define (dibbuk/addrmap-flags mp)
  (MapRange->flags mp))
(define (dibbuk/proc-maps pid)
  (ProcessMemoryMapping pid))
(define (dibbuk/read-mem pid addr len)
  (ReadProcMem addr len pid))

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

(define (radix-string->int s base)
  (rust.radix-string->int s base))

(define (int->hex s #:leading [leading 2])
  (rust.int->hex s leading))

(define (state-insert k v)
  (list (hash-insert *dibbuk-state* k v) (dibbuk/none)))

(define *dibbuk-command* (dibbuk/none))
(define *dibbuk-state* (hash 'verbose #false))
(define *rato-ui-str* (hash))

(define (rato/tick? r) (TermTick? r))
(define (rato/clear) (TerminalClear))

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

(define (dibbuk/handle-event state evt-str)
  (let ((result (handle-event state (string->jsexpr evt-str))))
    (set! *dibbuk-state* (first result))
    (set! *dibbuk-command* (second result))
    (if (hash-contains? *dibbuk-state* 'rato-ui)
      (set! *rato-ui-str* (value->jsexpr-string (hash-ref *dibbuk-state* 'rato-ui))))))

(define (dibbuk/hello)
  (set! *dibbuk-command* (dibbuk/cmd (list "a" "b"))))

(define (dibbuk/push-history state str)
  (letrec ((history (hash-get! state (list 'ui 'command-history) #:default (list))))
    (-> state
      (hash-join!
        (list 'ui)
        (hash
          'command-history
          (append history (list str))))
      (hash-join! (list 'ui 'history) (hash 'focus (+ 1 (length history)))))))

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
            (dibbuk/push-history (string-append "user> " str))
            (dibbuk/push-history (string-append "=> " (to-string expr-result))))
          (dibbuk/none))))
    ; just send to gdb instead
    (list
      (dibbuk/push-history state (string-append "user> " str))
      (dibbuk/cmd (list str)))))

(define (state-update state key value)
  (letrec ((old-substate (if (hash-contains? state key)
                          (hash-ref state key)
                          (hash)))
           (new-substate (hash-union value old-substate)))
    (hash-insert state key new-substate)))

(define (hash-get! h keys #:default [default void])
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
        (hash-insert h k new-substate)))
    ;
    ))

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
(define *debug-registers* (hashset "rax" "rbx" "rdi" "rbp" "rsp" "rdx" "rsi" "rip"))

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

(define (ui/memory state)
  (hash-join! state
    (list 'ui 'memory)
    (hash 'widget
      (if (and
           (hash-get! state (list 'register-state "rip") #:default #false)
           (hash-get! state (list 'target 'pid) #:default #false))
        (-> (dibbuk/read-mem
             (hash-get! state (list 'target 'pid))
             (hash-get! state (list 'register-state "rip"))
             16)
          (transduce (mapping (lambda (byte) (int->hex byte))) (into-list))
          (to-string)
          (list)
          (rato/list))
        (rato/list (list "nothing..."))))))

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
  ; (rato/clear)
  ; (debug-print-regs state)
  ; (displayln "\rHi")
  ;
  (letrec ((current-widget (hash-get! state (list 'ui 'current-widget) #:default 'history))
           (updated-state
             (->
               state
               (ui/registers)
               (ui/memory)
               (ui/history))))

    (list (hash-insert
           updated-state
           'rato-ui
           ; (hash-get!
           ;   updated-state
           ;   (list 'ui current-widget 'widget)
           ;   #:default
           ;   (rato/paragraph "loading..."))
           (rato/ui (list
                     (hash-get! updated-state (list 'ui 'history 'widget) #:default (rato/paragraph "...ops"))
                     (hash-get! updated-state (list 'ui 'registers 'widget) #:default (rato/paragraph "...ops"))
                     (hash-get! updated-state (list 'ui 'memory 'widget) #:default (rato/paragraph "...ops"))
                     (rato/paragraph "bottom")
                     ;
                     )
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
                (dibbuk/push-history state (to-string s))
                (dibbuk/none)))

            ((LogStream s)
              ; Not sure what the semantics of a LogStream are on GDB MI, but it returns the user input here, so I'm doing nothing with it to not repeat the user input
              ; (displayln (trim-end-matches s "\n"))
              (list
                (dibbuk/push-history state s)
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
                        (dibbuk/push-history (string-append
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
                             (dibbuk/push-history (string-append "THREADS> " (to-string id)))
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
        (dibbuk/push-history "[debug] event:")
        (dibbuk/push-history (to-string event))
        (dibbuk/push-history "[debug] result:")
        (dibbuk/push-history (to-string result))
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
