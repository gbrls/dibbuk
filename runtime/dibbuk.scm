;
(define (dibbuk/cmd l) (GdbCommandsReq l))
(define (dibbuk/none) (EmptyReq))
(define (dibbuk/reload) (Reload))
(define (dibbuk/req? r) (Request? r))

(define *dibbuk-command* (dibbuk/none))
(define *dibbuk-state* (hash))

(define (state-insert k v)
  (list (hash-insert *dibbuk-state* k v) (dibbuk/none)))

(define (dibbuk/handle-event state evt-str)
  (let ((result (handle-event state (string->jsexpr evt-str))))
    (set! *dibbuk-state* (first result))
    (set! *dibbuk-command* (second result))))

(define (dibbuk/hello)
  (set! *dibbuk-command* (dibbuk/cmd (list "a" "b"))))

(define (eval-user-input str)
  (if (starts-with? str ":")

    (let ((expr-result (->
                        str
                        (trim-start-matches ":")
                        (eval-string))))
      (displayln "=>" expr-result)
      expr-result)

    (dibbuk/cmd (list str))))

(define (handle-event state event)
  (let (
        (result
          (cond
            ((-> event hash? not) #false)

            ((hash-contains? event 'ConsoleStream)
              (displayln (->
                          event
                          (hash-ref 'ConsoleStream)
                          (trim-end-matches "\n"))))

            ((hash-contains? event 'LogStream)
              (displayln (->
                          event
                          (hash-ref 'LogStream)
                          (trim-end-matches "\n"))))

            ((hash-contains? event 'UserInput)
              (eval-user-input (hash-ref event 'UserInput)))

            ((hash-contains? event 'Result)
              (let ((result (->
                             event
                             (hash-ref 'Result)
                             (hash-ref 'results))))
                (cond
                  ((hash-contains? result 'register-names)
                    (let ((pairs
                            (->
                              result
                              (hash-ref 'register-names)
                              (hash-ref 'List)
                              (transduce (mapping (lambda (mp) (hash-ref mp 'Const))) (into-list))
                              (transduce (enumerating) (into-list))
                              ; (fold (lambda (x acc) (hash-insert acc (first x) (second x))))
                              )))
                      (list
                        (hash-insert *dibbuk-state* 'register-ids
                          (fold (lambda (x acc) (hash-insert acc (first x) (second x))) (hash) pairs))
                        (dibbuk/none))
                      ; (fold (lambda (x acc) x) pairs (hash))
                      ; pairs
                      ;
                      ))
                  ((hash-contains? result 'changed-registers)
                    (letrec ((ids
                               (->
                                 result
                                 (hash-ref 'changed-registers)
                                 (hash-ref 'List)
                                 (transduce (mapping (lambda (mp) (hash-ref mp 'Const))) (into-list))
                                 (string-join " ")))
                             (cmd-str (string-append "-data-list-register-values " ids)))
                      (begin
                        (displayln cmd-str)
                        (dibbuk/cmd (list cmd-str)))))
                  (#true result))))

            ((hash-contains? event 'NotifyAsync)
              (if (string=? "thread-group-started" (->
                                                    event
                                                    (hash-ref 'NotifyAsync)
                                                    (hash-ref 'class)))
                (dibbuk/cmd (list "-data-list-register-names"))
                (dibbuk/none)))

            ((hash-contains? event 'ExecAsync)
              (if (string=? "stopped" (->
                                       event
                                       (hash-ref 'ExecAsync)
                                       (hash-ref 'class)))
                (dibbuk/cmd (list "-data-list-changed-registers"))
                (dibbuk/none)))

            ((hash-contains? event 'StatusAsync)
              (->
                event
                (hash-ref 'StatusAsync)
                (hash-ref 'class)))

            ; default
            (#true #false))))

    ; verbose debug prints
    (if (and
         (hash-contains? *dibbuk-state* 'verbose)
         (hash-ref *dibbuk-state* 'verbose))
      (begin (displayln "result:")
        (displayln result)
        (displayln "====\n")))

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
