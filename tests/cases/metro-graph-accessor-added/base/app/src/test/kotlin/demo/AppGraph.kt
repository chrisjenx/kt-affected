package demo

import dev.zacsweers.metro.AppScope
import dev.zacsweers.metro.DependencyGraph

interface HasPricing {
    val pricing: Pricing
}

@DependencyGraph(AppScope::class)
interface AppGraph : HasPricing {
}
