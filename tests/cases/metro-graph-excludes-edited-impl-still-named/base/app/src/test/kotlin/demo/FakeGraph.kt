package demo

import dev.zacsweers.metro.AppScope
import dev.zacsweers.metro.DependencyGraph
import dev.zacsweers.metro.Provides

@DependencyGraph(AppScope::class, excludes = [PricingImpl::class])
interface FakeGraph : HasPricing {
    @Provides
    fun fakePricing(): Pricing = object : Pricing {
        override fun price(sku: String): Int = 1
        override fun currency(): String = "XXX"
    }
}
