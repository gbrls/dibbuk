(define *dibbuk-command* (EmptyReq))
(define *state* (hash))

(define (dibbuk/handle-event state evt-str)
  (let ((result (handle-event state (string->jsexpr evt-str))))
    (set! *state* (first result))
    (set! *dibbuk-command* (second result))))

(define (dibbuk/hello)
  (set! *dibbuk-command* (GdbCommandsReq (list "a" "b"))))

(define (eval-user-input str)
  (if (starts-with? str ":")

    (->
      str
      (trim-start-matches ":")
      (eval-string))

    (GdbCommandsReq (list str))))

(define (handle-event state event)
  (displayln event)
  (displayln "")
  (let (
        (parsed
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
              (->
                event
                (hash-ref 'Result)
                (hash-ref 'class)))

            ((hash-contains? event 'NotifyAsync)
              (if (string=? "thread-group-started" (->
                                                    event
                                                    (hash-ref 'NotifyAsync)
                                                    (hash-ref 'class)))
                (GdbCommandsReq (list "-data-list-register-names"))
                (EmptyReq)))

            ((hash-contains? event 'ExecAsync)
              (if (string=? "stopped" (->
                                       event
                                       (hash-ref 'ExecAsync)
                                       (hash-ref 'class)))
                (GdbCommandsReq (list "-data-list-changed-registers"))
                (EmptyReq)))

            ((hash-contains? event 'StatusAsync)
              (->
                event
                (hash-ref 'StatusAsync)
                (hash-ref 'class)))

            ; default
            (#true #false))))
    ; (displayln "parsed:")
    (displayln parsed)
    ; (displayln "====\n")
    (if (Request? parsed) (list state parsed)
      (list state (EmptyReq)))))

; Get PID
; thread-info
;
; Basic register commands to run on STOPPED
; data-list-changed-registers
; data-list-register-names
; data-list-register-values
;
