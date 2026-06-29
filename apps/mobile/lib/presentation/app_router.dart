import 'package:go_router/go_router.dart';

import 'crop_page.dart';
import 'generate_page.dart';
import 'home_page.dart';
import 'result_page.dart';

/// The four-screen offline flow: Home → Crop → Generate → Result.
final appRouter = GoRouter(
  routes: [
    GoRoute(path: '/', builder: (context, state) => const HomePage()),
    GoRoute(path: '/crop', builder: (context, state) => const CropPage()),
    GoRoute(
      path: '/generate',
      builder: (context, state) => const GeneratePage(),
    ),
    GoRoute(path: '/result', builder: (context, state) => const ResultPage()),
  ],
);
