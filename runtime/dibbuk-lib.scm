(define (dibbuk/disasm-map mp pid offset)
  (DisasmMapRange mp pid offset))

(define (dibbuk/disasm-at mp pid addr)
  (DisasmMapRangeOffset addr mp pid))

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

(define (dibbuk/hello)
  (set! *dibbuk-command* (dibbuk/cmd (list "a" "b"))))

(provide dibbuk/disasm-map)
(provide dibbuk/disasm-at)
(provide dibbuk/addrmap-contains?)
(provide dibbuk/addrmap-flags)
(provide dibbuk/proc-maps)
(provide dibbuk/read-mem)
(provide dibbuk/cmd)
(provide dibbuk/none)
(provide dibbuk/reload)
(provide dibbuk/req?)
(provide dibbuk/state-and-command?)
(provide dibbuk/next?)
(provide dibbuk/make-next)
(provide dibbuk/hello)
