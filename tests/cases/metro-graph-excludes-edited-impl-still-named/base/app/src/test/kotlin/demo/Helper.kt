package demo

object H1 { fun a() = H2.b() }
object H2 { fun b() = H3.c() }
object H3 { fun c() = H4.d() }
object H4 { fun d() = H5.e() }
object H5 { fun e() = H6.f() }
object H6 { fun f() = H7.g() }
object H7 { fun g() = PricingImpl().price("x") }
