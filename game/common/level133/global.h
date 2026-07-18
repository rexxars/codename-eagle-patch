/* Codename eagle Specific defines */
#define MAX_PLAYER_BULLETS 200
#define MAX_PLAYER_SHELLS 50
#define MAX_PLAYER_GAS 100

/* end: Codename eagle Specific defines */


#define PI 3.1415

#define CAMERA 0

#define CALL_NEXT_FRAME 0

#define THIS 0

#define TOPLAYER 0

#define TRUE 1
#define FALSE 0

/* REFGetProject(, Flags) */
#define GOURAUD_OFF 0
#define GOURAUD_ON 1

/* REFGetProVect(, Type, ) */
#define DOF 0
#define UP 1
#define RIGHT 2

#define XVAR 0
#define YVAR 1
#define ZVAR 2

/*HitItem(float OtherInx, float PlayerType)*/
/* float PlayerType */
#define HITITEM_LOCALPLAYER 0
#define HITITEM_OTHERPLAYER 1
#define HITITEM_VEHICLE 2

/* REFSetViewMode(ViewMode,,,) */
#define VIEW_INSIDE 0
#define VIEW_OUTSIDE 1
#define VIEW_FRONTFLYBY 2
#define VIEW_BEHINDCHASE 3


/* REFSetProjectVars()*/
#define ON 1
#define OFF 0

#define GRAVITY 0
#define MOVE 1
#define ROTATE 2
#define LANDCOLLISION 3
#define	OBJECTCOLLISION 4
#define IMMATERIAL 5
#define GLOWING 6
#define INSTANCEMOD 7
#define ITEM 8
#define MASS 9
#define CRUSHEDBYVEHICLE 10
#define AFFECTEDBYEXPLOSION 11
#define VISIBLE	12
#define NO_ZBUFFER 13
#define SOLID_MATERIAL 14
#define BOMB_PROJECTILE 15
#define IMMORTAL 16
#define WEAPON_TYPE 17
#define REMOVETRANSFACES 18
#define TYPE_PROJECTILE 19
#define TREEEXPLODEFUNCTION 20
#define PROJHEALTH 21

/* REFSetWeatherType() */
#define WEATHER_OFF 0
#define WEATHER_SNOW 1
#define WEATHER_RAIN 2

/* General */
#define MYSELF -1
 
/* AI Patrol */
#define START_PATROL 1
#define END_PATROL 0

/* SetLight */
#define SPOTLIGHT 0
#define NORMALLIGHT 1

/* REFChangePlayer(.., float Type, ....) */
#define PLAYER_HEALTH 0
#define PLAYER_AMMO 1
#define PLAYER_BULLETS 1
#define PLAYER_ARMOR 2
#define PLAYER_ABSORBS 3
#define PLAYER_GAS 4
#define PLAYER_SHELLS 5
#define PLAYER_FUEL 6
#define PLAYER_SETABSARMOR 7
#define PLAYER_CLIPBULLETS 8
#define PLAYER_CLIPSHELLS 9
#define PLAYER_CLIPGAS 10


/* REFSetAIVars -"- */
#define AI_LOOKFOR 0
#define AI_AWARE	 1
#define AI_MORALE	 2
#define AI_MODE		 3
#define AI_USE		 4
#define AI_TEAM    5
#define AI_SEERADIUS 6
#define AI_HEARRADIUS 7
#define AI_ATTACKRANGE 8
#define AI_DELETE 9
#define AI_AIMDEVIATION 10
#define AI_DROPBOMBS 11

/* REFSetAIVars - AI_MODE constants. */
#define AIMODE_ATTACK 1
#define AIMODE_PATROL 2
#define AIMODE_GUARD  4
#define AIMODE_FOLLOW 8
#define AIMODE_SPAWN 16

/* REFSetScore(float Team, float Mode, float Value) */
/* Team */
#define TEAMA 0
#define TEAMB 1
/* Mode */
#define FLAG_SCORE 0
#define KILLED_SCORE 1

/* Sounds */
#define FXDAMN 0
#define FXTELLMOM 1
#define FXDIE1 2
#define FXDIE2 3
#define FXHIT1 4
#define FXHIT2 5
#define FXHIT3 6
#define FXFENCE 7
#define FXSWITCH 8
#define FXCUT 9
#define FXRIFLE 10
#define FXRIFLE2 11
#define FXRICHO1 12
#define FXRICHO2 13
#define FXRICHO3 14
#define FXRICHO4 15
#define FXHALT1 16
#define FXSHOWPAPE 17
#define FXPASS 18
#define FXTHANKS1 19
#define FXFIRE1 20
#define FXALARM 21
#define FXBARK 22
#define FXEXPLOSION 23
#define FX_GDIE1 24
#define FX_GHIT1 25
#define FX_STEPS 26
#define FX_CAR 27
#define FX_IGNITION 28
#define FX_PLANE 29
#define FX_TANK 30
#define FX_CANNON 31
#define FX_HALT2 32
#define FX_GALARM 33
#define FX_HBARK2 34
#define FX_HBARK3 35
#define FX_DOGDIE1 36
#define FX_DOGDIE2 37
#define FX_WINDING 38
#define FX_WATFALL 40

#define DLG_SGANINTR 41
#define DLG_SGDROPYO 42
#define DLG_SGHALT 43
#define DLG_SGHALTPU 44
#define DLG_SGKILLHI 45
#define DLG_SGRAISET 46
#define DLG_SGRELEAS 47
#define DLG_SGSHOOTH 48
#define DLG_SGSHOOTR 49
#define DLG_SGSOUNDT 50
#define DLG_SGDESTRO 51

#define FX_OGATECLO 52
#define FX_OGATEEXP 53
#define FX_OGATEOPE 54
#define FX_ORADIOEX 55
#define FX_ORADIONO 56
#define FX_OENGELEC 57
#define FX_OENGGEN	58

/* REFLoadAnimation()*/
#define FIRSTFRAME 0
#define LASTFRAME 65535			/* 0xFFFF is now alloved in the scripts. No hex numbers */


