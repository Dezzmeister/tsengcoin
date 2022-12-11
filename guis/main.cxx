// generated by Fast Light User Interface Designer (fluid) version 1.0308

#include "main.h"

Fl_Menu_Item MainUI::menu_menu[] = {
 {"File", 0,  0, 0, 192, (uchar)FL_NORMAL_LABEL, 0, 14, 0},
 {"Settings", 0,  0, 0, 0, (uchar)FL_NORMAL_LABEL, 0, 14, 0},
 {0,0,0,0,0,0,0,0,0},
 {"New", 0,  0, 0, 192, (uchar)FL_NORMAL_LABEL, 0, 14, 0},
 {"Alias", 0,  0, 0, 0, (uchar)FL_NORMAL_LABEL, 0, 14, 0},
 {"Chat", 0,  0, 0, 0, (uchar)FL_NORMAL_LABEL, 0, 14, 0},
 {0,0,0,0,0,0,0,0,0},
 {"Help", 0,  0, 0, 192, (uchar)FL_NORMAL_LABEL, 0, 14, 0},
 {"About", 0,  0, 0, 0, (uchar)FL_NORMAL_LABEL, 0, 14, 0},
 {0,0,0,0,0,0,0,0,0},
 {0,0,0,0,0,0,0,0,0}
};

MainUI::MainUI() {
  Fl_Double_Window* w;
  { Fl_Double_Window* o = new Fl_Double_Window(400, 300, "TsengCoin");
    w = o; if (w) {/* empty */}
    o->user_data((void*)(this));
    { Fl_Menu_Bar* o = new Fl_Menu_Bar(0, 0, 400, 20, "menu");
      o->labeltype(FL_NO_LABEL);
      o->menu(menu_menu);
    } // Fl_Menu_Bar* o
    o->end();
    o->resizable(o);
  } // Fl_Double_Window* o
}