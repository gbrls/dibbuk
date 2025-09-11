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

(define (rato/tick? r) (TermTick? r))
(define (rato/clear) (TerminalClear))

(provide rato/layout)
(provide rato/layout-single)
(provide rato/layout-horizontal-2)
(provide rato/layout-vertical-2)
(provide rato/ui)
(provide rato/paragraph)
(provide rato/list)
(provide rato/tick?)
(provide rato/clear)
