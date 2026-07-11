import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'application/generate_settings.dart' show sharedPreferencesProvider;
import 'infrastructure/bead_ffi_loader.dart';
import 'l10n/app_localizations.dart';
import 'presentation/app_router.dart';
import 'presentation/theme.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // Load the native bead_ffi bridge before any generation can run (task 3.2).
  try {
    await initBeadFfi();
  } catch (e) {
    // ponytail: pre-l10n 崩溃兜底屏，AppLocalizations 未就绪，不纳入本地化（设计 D3）
    runApp(MaterialApp(
      home: Scaffold(body: Center(child: Text('引擎加载失败：$e'))),
    ));
    return;
  }
  // Pre-load persisted settings so the settings Notifier reads them synchronously
  // on the first frame — no not-ready window, no "default then overwrite" race
  // (design D4). Injected via override so the Notifier reads the ready instance.
  final SharedPreferences prefs;
  try {
    prefs = await SharedPreferences.getInstance();
  } catch (e) {
    // Degrade like the FFI path: a startup prefs failure shows a readable screen
    // instead of an unhandled crash. (The settings Notifier needs a ready prefs
    // instance to build, so we can't silently proceed without it.)
    // ponytail: pre-l10n 崩溃兜底屏，AppLocalizations 未就绪，不纳入本地化（设计 D3）
    runApp(MaterialApp(
      home: Scaffold(body: Center(child: Text('设置加载失败：$e'))),
    ));
    return;
  }
  runApp(ProviderScope(
    overrides: [sharedPreferencesProvider.overrideWithValue(prefs)],
    child: const BeadsmithApp(),
  ));
}

class BeadsmithApp extends StatelessWidget {
  const BeadsmithApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp.router(
      onGenerateTitle: (ctx) => AppLocalizations.of(ctx).appTitle,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      theme: lightTheme,
      darkTheme: darkTheme,
      themeMode: ThemeMode.system,
      routerConfig: appRouter,
    );
  }
}
