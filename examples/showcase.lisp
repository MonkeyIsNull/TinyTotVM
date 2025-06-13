(define x 10)
(define y 20)

(print (+ x y))          ; Should print 30
(print (> y x))          ; Should print true
(print (< x y))          ; Should print true

(if (= x y)
    (print "Equal")
    (print "Not equal"))

(set! x 20)

(if (= x y)
    (print "Now equal!")
    (print "Still not equal"))

