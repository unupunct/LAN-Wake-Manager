package com.lanwakemanager.app.ui

import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextOverflow
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.lanwakemanager.app.api.ConnMode

private data class Tab(val route: String, val label: String, val icon: ImageVector)

private val tabs = listOf(
    Tab("home", "Home", Icons.Filled.Home),
    Tab("computers", "Computers", Icons.Filled.DesktopWindows),
    Tab("locations", "Locations", Icons.Filled.Place),
    Tab("schedules", "Schedules", Icons.Filled.Schedule),
    Tab("more", "More", Icons.Filled.Menu),
)

private val moreRoutes = setOf("groups", "history", "diagnostics", "settings", "group_edit/{id}")

private fun titleFor(route: String?): String = when {
    route == null -> "LAN Wake Manager"
    route == "home" -> "LAN Wake Manager"
    route == "computers" -> "Computers"
    route == "locations" -> "Locations"
    route == "schedules" -> "Schedules"
    route == "more" -> "More"
    route == "groups" -> "Groups"
    route == "history" -> "History"
    route == "diagnostics" -> "Diagnostics"
    route == "settings" -> "Settings"
    route == "add_controller" -> "Add controller"
    route.startsWith("device") -> "Computer"
    route.startsWith("schedule_edit") -> "Schedule"
    route.startsWith("group_edit") -> "Group"
    else -> "LAN Wake Manager"
}

/** Root composable: navigation + bottom bar. Stateless w.r.t. data: everything comes from [s] and [actions]. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LwmRoot(
    s: AppUiState,
    actions: AppActions,
    pairing: PairingUiState,
    pairingActions: PairingActions,
    onScanQr: () -> Unit,
    snackbarHost: SnackbarHostState,
    nav: NavHostController = rememberNavController(),
) {
    val entry by nav.currentBackStackEntryAsState()
    val route = entry?.destination?.route
    val isTab = route in tabs.map { it.route }
    val showBottom = isTab || route in moreRoutes || route == "diagnostics"
    fun goTab(r: String) = nav.navigate(r) {
        popUpTo(nav.graph.findStartDestination().id) { saveState = true }
        launchSingleTop = true; restoreState = true
    }
    val addController = { pairingActions.reset(); nav.navigate("add_controller") { launchSingleTop = true } }

    LwmFrame(
        route = route, isTab = isTab, showBottom = showBottom, canRefresh = s.active != null,
        onTab = ::goTab, onBack = { nav.popBackStack() }, onRefresh = actions::refresh, snackbarHost = snackbarHost,
    ) { pad ->
        NavHost(nav, startDestination = "home", modifier = Modifier.padding(pad)) {
            composable("home") { HomeScreen(s, actions, onNavigate = { r -> if (r in tabs.map { it.route }) goTab(r) else nav.navigate(r) }, onAddController = addController) }
            composable("computers") { ComputersScreen(s, actions, onDetails = { nav.navigate("device/$it") }, onAddController = addController) }
            composable("locations") { LocationsScreen(s, actions, onAddController = addController) }
            composable("schedules") { SchedulesScreen(s, actions, onEdit = { nav.navigate("schedule_edit/${it ?: "new"}") }, onAddController = addController) }
            composable("more") { MoreScreen(onNavigate = { nav.navigate(it) { launchSingleTop = true } }, s = s) }
            composable("groups") { GroupsScreen(s, actions, onEdit = { nav.navigate("group_edit/${it ?: "new"}") }, onAddController = addController) }
            composable("history") { HistoryScreen(s, actions, onAddController = addController) }
            composable("diagnostics") { DiagnosticsScreen(s, actions, onRepair = addController) }
            composable("settings") { SettingsScreen(s, actions, onAddController = addController) }
            composable("add_controller") {
                AddControllerScreen(pairing, pairingActions, s.settings.connectionMode, onScan = onScanQr, onDone = {
                    pairingActions.reset(); nav.navigate("home") { popUpTo("home") { inclusive = true } }
                })
            }
            composable("device/{id}", listOf(navArgument("id") { type = NavType.StringType })) { e ->
                DeviceDetailScreen(s, e.arguments?.getString("id").orEmpty(), actions)
            }
            composable("schedule_edit/{id}", listOf(navArgument("id") { type = NavType.StringType })) { e ->
                val id = e.arguments?.getString("id")?.takeIf { it != "new" }
                ScheduleEditScreen(s, id, actions, onDone = { nav.popBackStack() })
            }
            composable("group_edit/{id}", listOf(navArgument("id") { type = NavType.StringType })) { e ->
                val id = e.arguments?.getString("id")?.takeIf { it != "new" }
                GroupEditScreen(s, id, actions, onDone = { nav.popBackStack() })
            }
        }
    }
}

/** Top bar + bottom navigation + snackbar around a screen. Used by [LwmRoot] and by the snapshot tests. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LwmFrame(
    route: String?, isTab: Boolean, showBottom: Boolean, canRefresh: Boolean,
    onTab: (String) -> Unit, onBack: () -> Unit, onRefresh: () -> Unit, snackbarHost: SnackbarHostState,
    content: @Composable (PaddingValues) -> Unit,
) {
    Scaffold(
        snackbarHost = { SnackbarHost(snackbarHost) },
        topBar = {
            TopAppBar(
                title = { Text(titleFor(route), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = { if (!isTab) IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
                actions = { if (isTab && canRefresh) IconButton(onClick = onRefresh) { Icon(Icons.Filled.Refresh, "Refresh") } },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.background),
            )
        },
        bottomBar = {
            if (showBottom) NavigationBar(containerColor = MaterialTheme.colorScheme.surface) {
                tabs.forEach { t ->
                    val selected = route == t.route || (t.route == "more" && route in moreRoutes)
                    NavigationBarItem(selected = selected, onClick = { onTab(t.route) }, icon = { Icon(t.icon, t.label) }, label = { Text(t.label, maxLines = 1, fontSize = androidx.compose.ui.unit.TextUnit(11f, androidx.compose.ui.unit.TextUnitType.Sp)) })
                }
            }
        },
        containerColor = MaterialTheme.colorScheme.background,
        content = content,
    )
}
