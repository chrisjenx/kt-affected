package demo.orders

interface OrderRepo {
    fun find(id: String): String?
}

@ContributesBinding(AppScope::class)
class SqlOrderRepo : OrderRepo {
    override fun find(id: String): String? = null
}

class LeftoverOrderRepo : OrderRepo {
    override fun find(id: String): String? = id
}

object AppScope

annotation class ContributesBinding(val scope: kotlin.reflect.KClass<*>)

annotation class Provides

object ClockModule {
    @Provides
    fun clock(): Long = 0L
}
