package demo

import dev.zacsweers.metro.Inject

@Inject
class Audit {
    fun record(event: String): String = "audit:$event"
}
