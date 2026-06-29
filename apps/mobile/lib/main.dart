import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'infrastructure/bead_ffi_loader.dart';
import 'presentation/app_router.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // Load the native bead_ffi bridge before any generation can run (task 3.2).
  try {
    await initBeadFfi();
  } catch (e) {
    runApp(MaterialApp(
      home: Scaffold(body: Center(child: Text('引擎加载失败：$e'))),
    ));
    return;
  }
  runApp(const ProviderScope(child: BeadsmithApp()));
}

class BeadsmithApp extends StatelessWidget {
  const BeadsmithApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp.router(
      title: 'Beadsmith',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.deepPurple),
      ),
      routerConfig: appRouter,
    );
  }
}
