package demo

@DependencyGraph(AppScope::class)
interface WiredGraph : HasSchedule

fun makeWiredGraph(): WiredGraph = TODO()
