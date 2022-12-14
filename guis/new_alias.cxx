// generated by Fast Light User Interface Designer (fluid) version 1.0308

#include "new_alias.h"

NewAlias::NewAlias() {
  Fl_Double_Window* w;
  { Fl_Double_Window* o = new Fl_Double_Window(400, 150, "New Alias");
    w = o; if (w) {/* empty */}
    o->user_data((void*)(this));
    { new Fl_Button(325, 120, 64, 20, "Save");
    } // Fl_Button* o
    { Fl_Input* o = new Fl_Input(20, 32, 225, 22, "Address");
      o->align(Fl_Align(FL_ALIGN_TOP_LEFT));
    } // Fl_Input* o
    { Fl_Input* o = new Fl_Input(20, 81, 225, 22, "Alias");
      o->align(Fl_Align(FL_ALIGN_TOP_LEFT));
    } // Fl_Input* o
    { Fl_Text_Display* o = new Fl_Text_Display(20, 120, 225, 20);
      o->color(FL_DARK3);
      o->labeltype(FL_NO_LABEL);
      o->textcolor((Fl_Color)1);
    } // Fl_Text_Display* o
    { new Fl_Button(255, 120, 64, 20, "Cancel");
    } // Fl_Button* o
    o->set_modal();
    o->end();
  } // Fl_Double_Window* o
}
