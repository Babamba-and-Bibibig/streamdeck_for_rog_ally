//! Small original vector icons: no image downloads, fonts or private icon files.
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use orangedeck_domain::Shortcut;

pub fn draw(p: &egui::Painter, center: Pos2, size: f32, color: Color32, action: Shortcut) {
    let point = |x: f32, y: f32| center + Vec2::new(x, y) * size / 24.0;
    let stroke = Stroke::new((size / 14.0).max(1.5), color);
    let line = |points: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            points.iter().map(|(x, y)| point(*x, *y)).collect(),
            stroke,
        ));
    };
    match action {
        Shortcut::Unassigned => {
            line(&[(-7., 0.), (7., 0.)]);
            line(&[(0., -7.), (0., 7.)]);
        }
        Shortcut::Live => line(&[
            (-11., 2.),
            (-7., 2.),
            (-4., -8.),
            (0., 9.),
            (4., -4.),
            (7., 2.),
            (11., 2.),
        ]),
        Shortcut::Projects | Shortcut::OpenProject => {
            line(&[
                (-10., 8.),
                (-10., -7.),
                (-3., -7.),
                (0., -4.),
                (10., -4.),
                (10., 8.),
                (-10., 8.),
            ]);
            if action == Shortcut::OpenProject {
                line(&[(-4., 2.), (5., 2.), (2., -1.)]);
            }
        }
        Shortcut::Conversations => line(&[
            (-10., -8.),
            (10., -8.),
            (10., 6.),
            (0., 6.),
            (-6., 10.),
            (-6., 6.),
            (-10., 6.),
            (-10., -8.),
        ]),
        Shortcut::Notifications => {
            line(&[
                (-9., 6.),
                (-6., 1.),
                (-6., -4.),
                (-3., -8.),
                (3., -8.),
                (6., -4.),
                (6., 1.),
                (9., 6.),
                (-9., 6.),
            ]);
            line(&[(-2., 10.), (2., 10.)]);
        }
        Shortcut::Refresh | Shortcut::FollowLatest => {
            line(&[
                (-8., -1.),
                (-8., -5.),
                (-4., -9.),
                (3., -9.),
                (8., -5.),
                (9., 0.),
            ]);
            line(&[(5., -2.), (9., 0.), (11., -4.)]);
            line(&[(8., 4.), (4., 9.), (-3., 9.), (-8., 5.)]);
            line(&[(-4., 5.), (-8., 5.), (-8., 9.)]);
            if action == Shortcut::FollowLatest {
                p.circle_filled(center, size * 0.09, color);
            }
        }
        Shortcut::PreviousProject | Shortcut::PreviousConversation => {
            line(&[(3., -8.), (-5., 0.), (3., 8.)]);
            if action == Shortcut::PreviousProject {
                line(&[(-10., -8.), (-10., 8.)]);
            }
        }
        Shortcut::NextProject | Shortcut::NextConversation => {
            line(&[(-3., -8.), (5., 0.), (-3., 8.)]);
            if action == Shortcut::NextProject {
                line(&[(10., -8.), (10., 8.)]);
            }
        }
        Shortcut::OpenEditor => {
            line(&[(-5., -7.), (-11., 0.), (-5., 7.)]);
            line(&[(5., -7.), (11., 0.), (5., 7.)]);
            line(&[(2., -10.), (-2., 10.)]);
        }
        Shortcut::OpenTerminal => {
            p.rect_stroke(
                Rect::from_center_size(center, Vec2::new(size, size * 0.78)),
                4,
                stroke,
                egui::StrokeKind::Inside,
            );
            line(&[(-7., -4.), (-2., 0.), (-7., 4.)]);
            line(&[(1., 4.), (7., 4.)]);
        }
        Shortcut::OpenBrowser => {
            p.circle_stroke(center, size * 0.43, stroke);
            p.add(egui::Shape::ellipse_stroke(
                center,
                Vec2::new(size * 0.18, size * 0.43),
                stroke,
            ));
            line(&[(-9., 0.), (9., 0.)]);
        }
    }
}
