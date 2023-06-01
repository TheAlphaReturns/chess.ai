#![allow(clippy::unwrap_used)]

use crossterm::{
    event::{KeyEvent, MouseEvent},
    terminal::size,
};

use intuitive::{
    components::{self, *},
    element,
    event::handler::Propagate,
    state::use_state,
    style::{Color as IColor, Modifier, Style},
    *,
};

use crate::{
    core::{
        color::Color,
        hexapawn::{Event, HexapawnBoard},
    },
    grbl,
    utils::string_builder::StringBuilder,
};

#[derive(Default)]
pub struct Root {
    starting_board: HexapawnBoard,
}

impl Root {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> components::Any {
        Self::default().into()
    }

    pub fn with_board(board: HexapawnBoard) -> components::Any {
        Self {
            starting_board: board,
        }
        .into()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Selection {
    Pc(usize, usize),
    Mv(usize, usize, usize, usize),
}

use Selection::*;

impl Component for Root {
    fn render(&self) -> element::Any {
        let game = use_state(|| self.starting_board.clone());
        let selection = use_state(|| Selection::Pc(0, 0));
        let error_message = use_state(StringBuilder::default);
        let helper_text = use_state(StringBuilder::default);
        let freeze = use_state(|| false);
        let counter = use_state(|| 499);

        let key_handler = {
            let game = game.clone();
            let selection = selection.clone();
            let helper_text = helper_text.clone();
            let error_message = error_message.clone();
            let freeze = freeze.clone();

            move |event: KeyEvent| {
                use intuitive::event::KeyCode::*;

                match event.code {
                    Char('q') => event::quit(),
                    _ => {}
                }

                if freeze.get() {
                    return Propagate::Next;
                }

                match event.code {
                    Char('q') => event::quit(),
                    Char('w') | Up => selection.mutate(|s| {
                        *s = match s {
                            Pc(x, y) => Pc(*x, ((*y as i32) - 1).max(0).min(2) as usize),
                            Mv(a, b, x, y) => {
                                Mv(*a, *b, *x, ((*y as i32) - 1).max(0).min(2) as usize)
                            }
                        }
                    }),
                    Char('s') | Down => selection.mutate(|s| {
                        *s = match s {
                            Pc(x, y) => Pc(*x, (*y + 1).max(0).min(2)),
                            Mv(a, b, x, y) => Mv(*a, *b, *x, (*y + 1).max(0).min(2)),
                        }
                    }),
                    Char('a') | Left => selection.mutate(|s| {
                        *s = match s {
                            Pc(x, y) => {
                                Pc(((*x as i32) - 1).max(0).min(2) as usize, (*y).max(0).min(2))
                            }
                            Mv(a, b, x, y) => Mv(
                                *a,
                                *b,
                                ((*x as i32) - 1).max(0).min(2) as usize,
                                (*y).max(0).min(2),
                            ),
                        }
                    }),
                    Char('d') | Right => selection.mutate(|s| {
                        *s = match s {
                            Pc(x, y) => Pc((*x + 1).max(0).min(2), (*y).max(0).min(2)),
                            Mv(a, b, x, y) => {
                                Mv(*a, *b, (*x + 1).max(0).min(2), (*y).max(0).min(2))
                            }
                        }
                    }),
                    Enter => match selection.get() {
                        Pc(x, y) => {
                            let board = game.get();

                            helper_text.mutate(|t| t.clear());

                            if let Some(piece) = board.at(x, y) {
                                if piece.c != board.turn {
                                    error_message.mutate(|e| {
                                        e.clear();
                                        e.addln("Error: not your turn");
                                    });
                                    return Propagate::Next;
                                }
                            } else {
                                error_message.mutate(|e| {
                                    e.clear();
                                    e.addln("Error: No piece at selected location")
                                });
                                return Propagate::Next;
                            }

                            selection.set(Mv(x, y, x, y));

                            error_message.mutate(|e| e.clear());
                        }
                        Mv(a, b, x, y) => {
                            game.mutate(|hp| {
                                let selected_move = match hp
                                    .get_moves()
                                    .iter()
                                    .find(|m| m.0 == a && m.1 == b && m.2 == x && m.3 == y)
                                {
                                    Some(pos) => *pos,
                                    None => {
                                        error_message.mutate(|e| {
                                            e.clear();
                                            e.addln("Error: invalid move");
                                        });
                                        return;
                                    }
                                };

                                match hp.make_move(selected_move) {
                                    Ok(_) => {}
                                    Err(err) => {
                                        error_message.mutate(|e| {
                                            e.clear();
                                            e.pushln(format!(
                                                "Error: invalid move: {}",
                                                err.to_string()
                                            ));
                                        });
                                        return;
                                    }
                                }

                                hp.turn.flip();

                                hp.event_emitter.emit(Event::Move, selected_move.clone());

                                selection.set(Pc(a, b));
                                error_message.mutate(|e| e.clear());
								

								if grbl::connected() {
									freeze.set(true);
								}
							});
                        }
                    },
                    Esc => {
                        error_message.mutate(|e| e.clear());

                        selection.set(match selection.get() {
                            Pc(x, y) => Pc(x, y),
                            Mv(a, b, _, _) => Pc(a, b),
                        })
                    }
                    _ => {}
                };

                Propagate::Next
            }
        };

        let mouse_handler = move |_: MouseEvent| Propagate::Stop;

        // prerender section
        let mut direction = helper_text.get().to_string();
        let (term_w, term_h) = size().unwrap();

        let min_term_w = 100;
        let min_term_h = 28;
        let size_ok = term_w >= min_term_w && term_h >= min_term_h;

        let flex = if size_ok {
            let mut wasdtext: String = String::default();
            wasdtext += "WASD/Arrow Keys to move";
            wasdtext += "\nEnter to select move/piece";
            wasdtext += "\nq to quit";
            direction = format!("{}\n{}\n{}", wasdtext, direction, error_message.get());

            (50, term_w - 50, 26, term_h - 26)
        } else {
            direction = String::new();
            direction += "Increase terminal size";

            (0, 1, 1, 0)
        };

        let board = game.get();
        let tos = board
            .get_moves()
            .iter()
            .filter_map(|(x1, y1, x2, y2, _)| match selection.get() {
                Pc(_, _) => None,
                Mv(a, b, _, _) if a == *x1 && b == *y1 => Some((*x2, *y2)),
                _ => None,
            })
            .collect::<Vec<_>>();

        if let Some(color) = board.is_win() {
            direction = format!("{} wins!\nq to quit", color);

            if !freeze.get() {
                freeze.set(true);
            }
        }

        if counter.get() == 500 || helper_text.get().to_string().is_empty() {
			if grbl::connected() && let Ok(s) = grbl::status()  {
				if s == grbl::Status::Working {
					helper_text.mutate(|t| {
						t.clear();
						t.addln("Running...");
					});
				} else {
					helper_text.mutate(|t| {
						t.clear();
						t.addln("Ready");
					});
	
					freeze.set(false);
				}
			}

			counter.set(0);
		} else {
			counter.mutate(|c| *c += 1);
		}

        render! {
            VStack(on_key: key_handler, on_mouse: mouse_handler, flex: [flex.2, flex.3]) {
                HStack(flex: [flex.0, flex.1]) {
                    Section(title: "Board") {
                        HStack() {
                            VStack() {
                                Section(border: match selection.get() {
                                    Pc(x, y) if x==0 && y==0 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    Mv(a,b,_,_) if a==0 && b==0 => Style::new(Some(IColor::Blue), None, Modifier::empty()),
                                    Mv(_,_,x,y) if x==0 && y==0 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    _ if tos.contains(&(0,0)) => Style::new(Some(IColor::Yellow), None, Modifier::empty()),
                                    _ => Style::default()
                                }) {
                                    HStack() {
                                        Text(text: board.at(0, 0).map_or(" ", |p| match p.c {
                                            Color::Black => "Black Pawn",
                                            Color::White => "White Pawn"
                                        }))
                                    }
                                }
                                Section(border: match selection.get() {
                                    Pc(x, y) if x==0 && y==1 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    Mv(a,b,_,_) if a==0 && b==1 => Style::new(Some(IColor::Blue), None, Modifier::empty()),
                                    Mv(_,_,x,y) if x==0 && y==1 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    _ if tos.contains(&(0,1)) => Style::new(Some(IColor::Yellow), None, Modifier::empty()),
                                    _ => Style::default()
                                }) {
                                    HStack() {
                                        Text(text: board.at(0, 1).map_or(" ", |p| match p.c {
                                            Color::Black => "Black Pawn",
                                            Color::White => "White Pawn"
                                        }))
                                    }
                                }
                                Section(border: match selection.get() {
                                    Pc(x, y) if x==0 && y==2 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    Mv(a,b,_,_) if a==0 && b==2 => Style::new(Some(IColor::Blue), None, Modifier::empty()),
                                    Mv(_,_,x,y) if x==0 && y==2 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    _ if tos.contains(&(0,2)) => Style::new(Some(IColor::Yellow), None, Modifier::empty()),
                                    _ => Style::default()
                                }) {
                                    HStack() {
                                        Text(text: board.at(0, 2).map_or(" ", |p| match p.c {
                                            Color::Black => "Black Pawn",
                                            Color::White => "White Pawn"
                                        }))
                                    }
                                }
                            }

                            VStack() {
                                Section(border: match selection.get() {
                                    Pc(x, y) if x==1 && y==0 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    Mv(a,b,_,_) if a==1 && b==0 => Style::new(Some(IColor::Blue), None, Modifier::empty()),
                                    Mv(_,_,x,y) if x==1 && y==0 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    _ if tos.contains(&(1,0)) => Style::new(Some(IColor::Yellow), None, Modifier::empty()),
                                    _ => Style::default()
                                }) {
                                    HStack() {
                                        Text(text: board.at(1, 0).map_or(" ", |p| match p.c {
                                            Color::Black => "Black Pawn",
                                            Color::White => "White Pawn"
                                        }))
                                    }
                                }
                                Section(border: match selection.get() {
                                    Pc(x, y) if x==1 && y==1 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    Mv(a,b,_,_) if a==1 && b==1 => Style::new(Some(IColor::Blue), None, Modifier::empty()),
                                    Mv(_,_,x,y) if x==1 && y==1 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    _ if tos.contains(&(1,1)) => Style::new(Some(IColor::Yellow), None, Modifier::empty()),
                                    _ => Style::default()
                                }) {
                                    HStack() {
                                        Text(text: board.at(1, 1).map_or(" ", |p| match p.c {
                                            Color::Black => "Black Pawn",
                                            Color::White => "White Pawn"
                                        }))
                                    }
                                }
                                Section(border: match selection.get() {
                                    Pc(x, y) if x==1 && y==2 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    Mv(a,b,_,_) if a==1 && b==2 => Style::new(Some(IColor::Blue), None, Modifier::empty()),
                                    Mv(_,_,x,y) if x==1 && y==2 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    _ if tos.contains(&(1,2)) => Style::new(Some(IColor::Yellow), None, Modifier::empty()),
                                    _ => Style::default()
                                }) {
                                    HStack() {
                                        Text(text: board.at(1, 2).map_or(" ", |p| match p.c {
                                            Color::Black => "Black Pawn",
                                            Color::White => "White Pawn"
                                        }))
                                    }
                                }
                            }

                            VStack() {
                                Section(border: match selection.get() {
                                    Pc(x, y) if x==2 && y==0 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    Mv(a,b,_,_) if a==2 && b==0 => Style::new(Some(IColor::Blue), None, Modifier::empty()),
                                    Mv(_,_,x,y) if x==2 && y==0 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    _ if tos.contains(&(2,0)) => Style::new(Some(IColor::Yellow), None, Modifier::empty()),
                                    _ => Style::default()
                                }) {
                                    HStack() {
                                        Text(text: board.at(2, 0).map_or(" ", |p| match p.c {
                                            Color::Black => "Black Pawn",
                                            Color::White => "White Pawn"
                                        }))
                                    }
                                }
                                Section(border: match selection.get() {
                                    Pc(x, y) if x==2 && y==1 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    Mv(a,b,_,_) if a==2 && b==1 => Style::new(Some(IColor::Blue), None, Modifier::empty()),
                                    Mv(_,_,x,y) if x==2 && y==1 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    _ if tos.contains(&(2,1)) => Style::new(Some(IColor::Yellow), None, Modifier::empty()),
                                    _ => Style::default()
                                }) {
                                    HStack() {
                                        Text(text: board.at(2, 1).map_or(" ", |p| match p.c {
                                            Color::Black => "Black Pawn",
                                            Color::White => "White Pawn"
                                        }))
                                    }
                                }
                                Section(border: match selection.get() {
                                    Pc(x, y) if x==2 && y==2 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    Mv(a,b,_,_) if a==2 && b==2 => Style::new(Some(IColor::Blue), None, Modifier::empty()),
                                    Mv(_,_,x,y) if x==2 && y==2 => Style::new(Some(IColor::Green), None, Modifier::empty()),
                                    _ if tos.contains(&(2,2)) => Style::new(Some(IColor::Yellow), None, Modifier::empty()),
                                    _ => Style::default()
                                }) {
                                    HStack() {
                                        Text(text: board.at(2, 2).map_or(" ", |p| match p.c {
                                            Color::Black => "Black Pawn",
                                            Color::White => "White Pawn"
                                        }))
                                    }
                                }
                            }
                        }
                    }

                    Section(title: "Instructions") {
                        Text(text: direction)
                    }
                }

                Empty()
            }
        }
    }
}
