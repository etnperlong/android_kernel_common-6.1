obj-$(CONFIG_MAC80211_HWSIM)	+= mac80211_hwsim.o
mac80211_hwsim-objs		+= mac80211_hwsim_main.o
mac80211_hwsim-objs		+= mac80211_hwsim_nan.o
mac80211_hwsim-$(CONFIG_NET_APF) += apf_interpreter.o
# Rename struct apf_info as mac80211_hwsim_main.c does, where it clashes with
# <net/apf.h>, and give the simulated chip 8 KiB of APF RAM shared by all hwsim
# interfaces.
CFLAGS_apf_interpreter.o := -include linux/export.h -Dapf_info=apf_fw_info \
			    -DAPF_STATE_MEMORY_TOTAL_LIMIT=8192

obj-$(CONFIG_VIRT_WIFI)	+= virt_wifi.o
