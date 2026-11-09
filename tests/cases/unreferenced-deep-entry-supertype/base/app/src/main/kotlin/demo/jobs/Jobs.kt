package demo.jobs

/** The framework's job type: it instantiates every subclass. */
interface Worker {
    fun work()
}

abstract class BaseWorker : Worker

abstract class SyncWorker : BaseWorker()

class OrderSync : SyncWorker() {
    override fun work() = Unit
}

class InvoiceFormatter {
    fun format(cents: Long): String = "$cents"
}
