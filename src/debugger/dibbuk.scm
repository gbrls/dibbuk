; (define log-data
;   (-> "./validation/mi-small.json"
;     open-input-file
;     read-port-to-string
;     string->jsexpr))

; λ > (fold (lambda (x y) (+ x y)) 2 '(3 4 5))
; => 14

; Get PID
; thread-info
;
; Basic register commands to run on STOPPED
; data-list-changed-registers
; data-list-register-names
; data-list-register-values
;
(define (dibbuk/handle-event state evt-str)
  (handle-event state (string->jsexpr evt-str)))

(define (dibbuk/hello)
  (displayln "hello from scheme!"))

(define (handle-event state event)
  (displayln event)
  (cond
    ((-> event hash? not) #false)

    ((hash-contains? event 'ConsoleStream)
      (string-append "GDB ~> " (hash-ref event 'ConsoleStream)))

    ((hash-contains? event 'LogStream)
      (string-append "user -> " (hash-ref event 'LogStream)))

    ((hash-contains? event 'Result)
      (-> event (hash-ref 'Result) (hash-ref 'class)))

    ((hash-contains? event 'NotifyAsync)
      (-> event (hash-ref 'NotifyAsync) (hash-ref 'class)))

    ((hash-contains? event 'ExecAsync)
      (-> event (hash-ref 'ExecAsync) (hash-ref 'class)))

    ((hash-contains? event 'StatusAsync)
      (-> event (hash-ref 'StatusAsync) (hash-ref 'class)))

    ; default
    (#true #false)))

; (define (get-mi log) (let ((i (car log)) (mi (handle-event 'any (last log)))) (displayln i mi)))

; (map get-mi
;   (-> log-data
;     (hash-ref 'gdb_output)))
