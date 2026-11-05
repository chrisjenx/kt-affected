package demo

class TestServices {
    val orders: OrderService = OrderService()
    val invoices: InvoiceService = InvoiceService()
}

class ServicesExtension {
    val services = TestServices()
}
