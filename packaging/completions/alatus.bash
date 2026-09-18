_alatus() {
    local i cur prev opts cmd
    COMPREPLY=()
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        cur="$2"
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
    fi
    prev="$3"
    cmd=""
    opts=""

    for i in "${COMP_WORDS[@]:0:COMP_CWORD}"
    do
        case "${cmd},${i}" in
            ",$1")
                cmd="alatus"
                ;;
            alatus,balanced)
                cmd="alatus__subcmd__balanced"
                ;;
            alatus,battery)
                cmd="alatus__subcmd__charge__subcmd__limit"
                ;;
            alatus,capabilities)
                cmd="alatus__subcmd__capabilities"
                ;;
            alatus,charge)
                cmd="alatus__subcmd__charge__subcmd__limit"
                ;;
            alatus,charge-limit)
                cmd="alatus__subcmd__charge__subcmd__limit"
                ;;
            alatus,completions)
                cmd="alatus__subcmd__completions"
                ;;
            alatus,cycle)
                cmd="alatus__subcmd__cycle"
                ;;
            alatus,daemon)
                cmd="alatus__subcmd__daemon"
                ;;
            alatus,display)
                cmd="alatus__subcmd__display"
                ;;
            alatus,fan)
                cmd="alatus__subcmd__fan"
                ;;
            alatus,full)
                cmd="alatus__subcmd__full"
                ;;
            alatus,gui)
                cmd="alatus__subcmd__gui"
                ;;
            alatus,help)
                cmd="alatus__subcmd__help"
                ;;
            alatus,mode)
                cmd="alatus__subcmd__mode"
                ;;
            alatus,monitor)
                cmd="alatus__subcmd__monitor"
                ;;
            alatus,oled)
                cmd="alatus__subcmd__oled"
                ;;
            alatus,perf)
                cmd="alatus__subcmd__perf"
                ;;
            alatus,performance)
                cmd="alatus__subcmd__perf"
                ;;
            alatus,power)
                cmd="alatus__subcmd__power"
                ;;
            alatus,profile)
                cmd="alatus__subcmd__mode"
                ;;
            alatus,quiet)
                cmd="alatus__subcmd__quiet"
                ;;
            alatus,rgb)
                cmd="alatus__subcmd__rgb"
                ;;
            alatus,session)
                cmd="alatus__subcmd__session"
                ;;
            alatus,status)
                cmd="alatus__subcmd__status"
                ;;
            alatus,thermal)
                cmd="alatus__subcmd__mode"
                ;;
            alatus,watch)
                cmd="alatus__subcmd__watch"
                ;;
            alatus__subcmd__daemon,help)
                cmd="alatus__subcmd__daemon__subcmd__help"
                ;;
            alatus__subcmd__daemon,session)
                cmd="alatus__subcmd__daemon__subcmd__session"
                ;;
            alatus__subcmd__daemon,status)
                cmd="alatus__subcmd__daemon__subcmd__status"
                ;;
            alatus__subcmd__daemon,system)
                cmd="alatus__subcmd__daemon__subcmd__system"
                ;;
            alatus__subcmd__daemon__subcmd__help,help)
                cmd="alatus__subcmd__daemon__subcmd__help__subcmd__help"
                ;;
            alatus__subcmd__daemon__subcmd__help,session)
                cmd="alatus__subcmd__daemon__subcmd__help__subcmd__session"
                ;;
            alatus__subcmd__daemon__subcmd__help,status)
                cmd="alatus__subcmd__daemon__subcmd__help__subcmd__status"
                ;;
            alatus__subcmd__daemon__subcmd__help,system)
                cmd="alatus__subcmd__daemon__subcmd__help__subcmd__system"
                ;;
            alatus__subcmd__daemon__subcmd__help__subcmd__session,restart)
                cmd="alatus__subcmd__daemon__subcmd__help__subcmd__session__subcmd__restart"
                ;;
            alatus__subcmd__daemon__subcmd__help__subcmd__session,run)
                cmd="alatus__subcmd__daemon__subcmd__help__subcmd__session__subcmd__run"
                ;;
            alatus__subcmd__daemon__subcmd__help__subcmd__session,start)
                cmd="alatus__subcmd__daemon__subcmd__help__subcmd__session__subcmd__start"
                ;;
            alatus__subcmd__daemon__subcmd__help__subcmd__session,status)
                cmd="alatus__subcmd__daemon__subcmd__help__subcmd__session__subcmd__status"
                ;;
            alatus__subcmd__daemon__subcmd__help__subcmd__session,stop)
                cmd="alatus__subcmd__daemon__subcmd__help__subcmd__session__subcmd__stop"
                ;;
            alatus__subcmd__daemon__subcmd__help__subcmd__system,restart)
                cmd="alatus__subcmd__daemon__subcmd__help__subcmd__system__subcmd__restart"
                ;;
            alatus__subcmd__daemon__subcmd__help__subcmd__system,run)
                cmd="alatus__subcmd__daemon__subcmd__help__subcmd__system__subcmd__run"
                ;;
            alatus__subcmd__daemon__subcmd__help__subcmd__system,start)
                cmd="alatus__subcmd__daemon__subcmd__help__subcmd__system__subcmd__start"
                ;;
            alatus__subcmd__daemon__subcmd__help__subcmd__system,status)
                cmd="alatus__subcmd__daemon__subcmd__help__subcmd__system__subcmd__status"
                ;;
            alatus__subcmd__daemon__subcmd__help__subcmd__system,stop)
                cmd="alatus__subcmd__daemon__subcmd__help__subcmd__system__subcmd__stop"
                ;;
            alatus__subcmd__daemon__subcmd__session,help)
                cmd="alatus__subcmd__daemon__subcmd__session__subcmd__help"
                ;;
            alatus__subcmd__daemon__subcmd__session,restart)
                cmd="alatus__subcmd__daemon__subcmd__session__subcmd__restart"
                ;;
            alatus__subcmd__daemon__subcmd__session,run)
                cmd="alatus__subcmd__daemon__subcmd__session__subcmd__run"
                ;;
            alatus__subcmd__daemon__subcmd__session,start)
                cmd="alatus__subcmd__daemon__subcmd__session__subcmd__start"
                ;;
            alatus__subcmd__daemon__subcmd__session,status)
                cmd="alatus__subcmd__daemon__subcmd__session__subcmd__status"
                ;;
            alatus__subcmd__daemon__subcmd__session,stop)
                cmd="alatus__subcmd__daemon__subcmd__session__subcmd__stop"
                ;;
            alatus__subcmd__daemon__subcmd__session__subcmd__help,help)
                cmd="alatus__subcmd__daemon__subcmd__session__subcmd__help__subcmd__help"
                ;;
            alatus__subcmd__daemon__subcmd__session__subcmd__help,restart)
                cmd="alatus__subcmd__daemon__subcmd__session__subcmd__help__subcmd__restart"
                ;;
            alatus__subcmd__daemon__subcmd__session__subcmd__help,run)
                cmd="alatus__subcmd__daemon__subcmd__session__subcmd__help__subcmd__run"
                ;;
            alatus__subcmd__daemon__subcmd__session__subcmd__help,start)
                cmd="alatus__subcmd__daemon__subcmd__session__subcmd__help__subcmd__start"
                ;;
            alatus__subcmd__daemon__subcmd__session__subcmd__help,status)
                cmd="alatus__subcmd__daemon__subcmd__session__subcmd__help__subcmd__status"
                ;;
            alatus__subcmd__daemon__subcmd__session__subcmd__help,stop)
                cmd="alatus__subcmd__daemon__subcmd__session__subcmd__help__subcmd__stop"
                ;;
            alatus__subcmd__daemon__subcmd__system,help)
                cmd="alatus__subcmd__daemon__subcmd__system__subcmd__help"
                ;;
            alatus__subcmd__daemon__subcmd__system,restart)
                cmd="alatus__subcmd__daemon__subcmd__system__subcmd__restart"
                ;;
            alatus__subcmd__daemon__subcmd__system,run)
                cmd="alatus__subcmd__daemon__subcmd__system__subcmd__run"
                ;;
            alatus__subcmd__daemon__subcmd__system,start)
                cmd="alatus__subcmd__daemon__subcmd__system__subcmd__start"
                ;;
            alatus__subcmd__daemon__subcmd__system,status)
                cmd="alatus__subcmd__daemon__subcmd__system__subcmd__status"
                ;;
            alatus__subcmd__daemon__subcmd__system,stop)
                cmd="alatus__subcmd__daemon__subcmd__system__subcmd__stop"
                ;;
            alatus__subcmd__daemon__subcmd__system__subcmd__help,help)
                cmd="alatus__subcmd__daemon__subcmd__system__subcmd__help__subcmd__help"
                ;;
            alatus__subcmd__daemon__subcmd__system__subcmd__help,restart)
                cmd="alatus__subcmd__daemon__subcmd__system__subcmd__help__subcmd__restart"
                ;;
            alatus__subcmd__daemon__subcmd__system__subcmd__help,run)
                cmd="alatus__subcmd__daemon__subcmd__system__subcmd__help__subcmd__run"
                ;;
            alatus__subcmd__daemon__subcmd__system__subcmd__help,start)
                cmd="alatus__subcmd__daemon__subcmd__system__subcmd__help__subcmd__start"
                ;;
            alatus__subcmd__daemon__subcmd__system__subcmd__help,status)
                cmd="alatus__subcmd__daemon__subcmd__system__subcmd__help__subcmd__status"
                ;;
            alatus__subcmd__daemon__subcmd__system__subcmd__help,stop)
                cmd="alatus__subcmd__daemon__subcmd__system__subcmd__help__subcmd__stop"
                ;;
            alatus__subcmd__display,dim)
                cmd="alatus__subcmd__display__subcmd__dim"
                ;;
            alatus__subcmd__display,help)
                cmd="alatus__subcmd__display__subcmd__help"
                ;;
            alatus__subcmd__display,mux)
                cmd="alatus__subcmd__display__subcmd__mux"
                ;;
            alatus__subcmd__display,od)
                cmd="alatus__subcmd__display__subcmd__overdrive"
                ;;
            alatus__subcmd__display,overdrive)
                cmd="alatus__subcmd__display__subcmd__overdrive"
                ;;
            alatus__subcmd__display,rate)
                cmd="alatus__subcmd__display__subcmd__rate"
                ;;
            alatus__subcmd__display,status)
                cmd="alatus__subcmd__display__subcmd__status"
                ;;
            alatus__subcmd__display__subcmd__help,dim)
                cmd="alatus__subcmd__display__subcmd__help__subcmd__dim"
                ;;
            alatus__subcmd__display__subcmd__help,help)
                cmd="alatus__subcmd__display__subcmd__help__subcmd__help"
                ;;
            alatus__subcmd__display__subcmd__help,mux)
                cmd="alatus__subcmd__display__subcmd__help__subcmd__mux"
                ;;
            alatus__subcmd__display__subcmd__help,overdrive)
                cmd="alatus__subcmd__display__subcmd__help__subcmd__overdrive"
                ;;
            alatus__subcmd__display__subcmd__help,rate)
                cmd="alatus__subcmd__display__subcmd__help__subcmd__rate"
                ;;
            alatus__subcmd__display__subcmd__help,status)
                cmd="alatus__subcmd__display__subcmd__help__subcmd__status"
                ;;
            alatus__subcmd__fan,balanced)
                cmd="alatus__subcmd__fan__subcmd__balanced"
                ;;
            alatus__subcmd__fan,cycle)
                cmd="alatus__subcmd__fan__subcmd__cycle"
                ;;
            alatus__subcmd__fan,full)
                cmd="alatus__subcmd__fan__subcmd__full"
                ;;
            alatus__subcmd__fan,get)
                cmd="alatus__subcmd__fan__subcmd__get"
                ;;
            alatus__subcmd__fan,help)
                cmd="alatus__subcmd__fan__subcmd__help"
                ;;
            alatus__subcmd__fan,performance)
                cmd="alatus__subcmd__fan__subcmd__performance"
                ;;
            alatus__subcmd__fan,quiet)
                cmd="alatus__subcmd__fan__subcmd__quiet"
                ;;
            alatus__subcmd__fan,set)
                cmd="alatus__subcmd__fan__subcmd__set"
                ;;
            alatus__subcmd__fan__subcmd__help,balanced)
                cmd="alatus__subcmd__fan__subcmd__help__subcmd__balanced"
                ;;
            alatus__subcmd__fan__subcmd__help,cycle)
                cmd="alatus__subcmd__fan__subcmd__help__subcmd__cycle"
                ;;
            alatus__subcmd__fan__subcmd__help,full)
                cmd="alatus__subcmd__fan__subcmd__help__subcmd__full"
                ;;
            alatus__subcmd__fan__subcmd__help,get)
                cmd="alatus__subcmd__fan__subcmd__help__subcmd__get"
                ;;
            alatus__subcmd__fan__subcmd__help,help)
                cmd="alatus__subcmd__fan__subcmd__help__subcmd__help"
                ;;
            alatus__subcmd__fan__subcmd__help,performance)
                cmd="alatus__subcmd__fan__subcmd__help__subcmd__performance"
                ;;
            alatus__subcmd__fan__subcmd__help,quiet)
                cmd="alatus__subcmd__fan__subcmd__help__subcmd__quiet"
                ;;
            alatus__subcmd__fan__subcmd__help,set)
                cmd="alatus__subcmd__fan__subcmd__help__subcmd__set"
                ;;
            alatus__subcmd__help,balanced)
                cmd="alatus__subcmd__help__subcmd__balanced"
                ;;
            alatus__subcmd__help,capabilities)
                cmd="alatus__subcmd__help__subcmd__capabilities"
                ;;
            alatus__subcmd__help,charge-limit)
                cmd="alatus__subcmd__help__subcmd__charge__subcmd__limit"
                ;;
            alatus__subcmd__help,completions)
                cmd="alatus__subcmd__help__subcmd__completions"
                ;;
            alatus__subcmd__help,cycle)
                cmd="alatus__subcmd__help__subcmd__cycle"
                ;;
            alatus__subcmd__help,daemon)
                cmd="alatus__subcmd__help__subcmd__daemon"
                ;;
            alatus__subcmd__help,display)
                cmd="alatus__subcmd__help__subcmd__display"
                ;;
            alatus__subcmd__help,fan)
                cmd="alatus__subcmd__help__subcmd__fan"
                ;;
            alatus__subcmd__help,full)
                cmd="alatus__subcmd__help__subcmd__full"
                ;;
            alatus__subcmd__help,gui)
                cmd="alatus__subcmd__help__subcmd__gui"
                ;;
            alatus__subcmd__help,help)
                cmd="alatus__subcmd__help__subcmd__help"
                ;;
            alatus__subcmd__help,mode)
                cmd="alatus__subcmd__help__subcmd__mode"
                ;;
            alatus__subcmd__help,monitor)
                cmd="alatus__subcmd__help__subcmd__monitor"
                ;;
            alatus__subcmd__help,oled)
                cmd="alatus__subcmd__help__subcmd__oled"
                ;;
            alatus__subcmd__help,perf)
                cmd="alatus__subcmd__help__subcmd__perf"
                ;;
            alatus__subcmd__help,power)
                cmd="alatus__subcmd__help__subcmd__power"
                ;;
            alatus__subcmd__help,quiet)
                cmd="alatus__subcmd__help__subcmd__quiet"
                ;;
            alatus__subcmd__help,rgb)
                cmd="alatus__subcmd__help__subcmd__rgb"
                ;;
            alatus__subcmd__help,session)
                cmd="alatus__subcmd__help__subcmd__session"
                ;;
            alatus__subcmd__help,status)
                cmd="alatus__subcmd__help__subcmd__status"
                ;;
            alatus__subcmd__help,watch)
                cmd="alatus__subcmd__help__subcmd__watch"
                ;;
            alatus__subcmd__help__subcmd__daemon,session)
                cmd="alatus__subcmd__help__subcmd__daemon__subcmd__session"
                ;;
            alatus__subcmd__help__subcmd__daemon,status)
                cmd="alatus__subcmd__help__subcmd__daemon__subcmd__status"
                ;;
            alatus__subcmd__help__subcmd__daemon,system)
                cmd="alatus__subcmd__help__subcmd__daemon__subcmd__system"
                ;;
            alatus__subcmd__help__subcmd__daemon__subcmd__session,restart)
                cmd="alatus__subcmd__help__subcmd__daemon__subcmd__session__subcmd__restart"
                ;;
            alatus__subcmd__help__subcmd__daemon__subcmd__session,run)
                cmd="alatus__subcmd__help__subcmd__daemon__subcmd__session__subcmd__run"
                ;;
            alatus__subcmd__help__subcmd__daemon__subcmd__session,start)
                cmd="alatus__subcmd__help__subcmd__daemon__subcmd__session__subcmd__start"
                ;;
            alatus__subcmd__help__subcmd__daemon__subcmd__session,status)
                cmd="alatus__subcmd__help__subcmd__daemon__subcmd__session__subcmd__status"
                ;;
            alatus__subcmd__help__subcmd__daemon__subcmd__session,stop)
                cmd="alatus__subcmd__help__subcmd__daemon__subcmd__session__subcmd__stop"
                ;;
            alatus__subcmd__help__subcmd__daemon__subcmd__system,restart)
                cmd="alatus__subcmd__help__subcmd__daemon__subcmd__system__subcmd__restart"
                ;;
            alatus__subcmd__help__subcmd__daemon__subcmd__system,run)
                cmd="alatus__subcmd__help__subcmd__daemon__subcmd__system__subcmd__run"
                ;;
            alatus__subcmd__help__subcmd__daemon__subcmd__system,start)
                cmd="alatus__subcmd__help__subcmd__daemon__subcmd__system__subcmd__start"
                ;;
            alatus__subcmd__help__subcmd__daemon__subcmd__system,status)
                cmd="alatus__subcmd__help__subcmd__daemon__subcmd__system__subcmd__status"
                ;;
            alatus__subcmd__help__subcmd__daemon__subcmd__system,stop)
                cmd="alatus__subcmd__help__subcmd__daemon__subcmd__system__subcmd__stop"
                ;;
            alatus__subcmd__help__subcmd__display,dim)
                cmd="alatus__subcmd__help__subcmd__display__subcmd__dim"
                ;;
            alatus__subcmd__help__subcmd__display,mux)
                cmd="alatus__subcmd__help__subcmd__display__subcmd__mux"
                ;;
            alatus__subcmd__help__subcmd__display,overdrive)
                cmd="alatus__subcmd__help__subcmd__display__subcmd__overdrive"
                ;;
            alatus__subcmd__help__subcmd__display,rate)
                cmd="alatus__subcmd__help__subcmd__display__subcmd__rate"
                ;;
            alatus__subcmd__help__subcmd__display,status)
                cmd="alatus__subcmd__help__subcmd__display__subcmd__status"
                ;;
            alatus__subcmd__help__subcmd__fan,balanced)
                cmd="alatus__subcmd__help__subcmd__fan__subcmd__balanced"
                ;;
            alatus__subcmd__help__subcmd__fan,cycle)
                cmd="alatus__subcmd__help__subcmd__fan__subcmd__cycle"
                ;;
            alatus__subcmd__help__subcmd__fan,full)
                cmd="alatus__subcmd__help__subcmd__fan__subcmd__full"
                ;;
            alatus__subcmd__help__subcmd__fan,get)
                cmd="alatus__subcmd__help__subcmd__fan__subcmd__get"
                ;;
            alatus__subcmd__help__subcmd__fan,performance)
                cmd="alatus__subcmd__help__subcmd__fan__subcmd__performance"
                ;;
            alatus__subcmd__help__subcmd__fan,quiet)
                cmd="alatus__subcmd__help__subcmd__fan__subcmd__quiet"
                ;;
            alatus__subcmd__help__subcmd__fan,set)
                cmd="alatus__subcmd__help__subcmd__fan__subcmd__set"
                ;;
            alatus__subcmd__help__subcmd__mode,auto)
                cmd="alatus__subcmd__help__subcmd__mode__subcmd__auto"
                ;;
            alatus__subcmd__help__subcmd__mode,balanced)
                cmd="alatus__subcmd__help__subcmd__mode__subcmd__balanced"
                ;;
            alatus__subcmd__help__subcmd__mode,cycle)
                cmd="alatus__subcmd__help__subcmd__mode__subcmd__cycle"
                ;;
            alatus__subcmd__help__subcmd__mode,full)
                cmd="alatus__subcmd__help__subcmd__mode__subcmd__full"
                ;;
            alatus__subcmd__help__subcmd__mode,get)
                cmd="alatus__subcmd__help__subcmd__mode__subcmd__get"
                ;;
            alatus__subcmd__help__subcmd__mode,performance)
                cmd="alatus__subcmd__help__subcmd__mode__subcmd__performance"
                ;;
            alatus__subcmd__help__subcmd__mode,quiet)
                cmd="alatus__subcmd__help__subcmd__mode__subcmd__quiet"
                ;;
            alatus__subcmd__help__subcmd__mode,set)
                cmd="alatus__subcmd__help__subcmd__mode__subcmd__set"
                ;;
            alatus__subcmd__help__subcmd__oled,dim)
                cmd="alatus__subcmd__help__subcmd__oled__subcmd__dim"
                ;;
            alatus__subcmd__help__subcmd__oled,refresh)
                cmd="alatus__subcmd__help__subcmd__oled__subcmd__refresh"
                ;;
            alatus__subcmd__help__subcmd__oled,status)
                cmd="alatus__subcmd__help__subcmd__oled__subcmd__status"
                ;;
            alatus__subcmd__help__subcmd__power,ppt)
                cmd="alatus__subcmd__help__subcmd__power__subcmd__ppt"
                ;;
            alatus__subcmd__help__subcmd__power__subcmd__ppt,list)
                cmd="alatus__subcmd__help__subcmd__power__subcmd__ppt__subcmd__list"
                ;;
            alatus__subcmd__help__subcmd__power__subcmd__ppt,set)
                cmd="alatus__subcmd__help__subcmd__power__subcmd__ppt__subcmd__set"
                ;;
            alatus__subcmd__help__subcmd__rgb,off)
                cmd="alatus__subcmd__help__subcmd__rgb__subcmd__off"
                ;;
            alatus__subcmd__help__subcmd__rgb,on)
                cmd="alatus__subcmd__help__subcmd__rgb__subcmd__on"
                ;;
            alatus__subcmd__help__subcmd__rgb,set-brightness)
                cmd="alatus__subcmd__help__subcmd__rgb__subcmd__set__subcmd__brightness"
                ;;
            alatus__subcmd__help__subcmd__rgb,set-color)
                cmd="alatus__subcmd__help__subcmd__rgb__subcmd__set__subcmd__color"
                ;;
            alatus__subcmd__help__subcmd__rgb,status)
                cmd="alatus__subcmd__help__subcmd__rgb__subcmd__status"
                ;;
            alatus__subcmd__help__subcmd__rgb,sync)
                cmd="alatus__subcmd__help__subcmd__rgb__subcmd__sync"
                ;;
            alatus__subcmd__help__subcmd__rgb,timeout)
                cmd="alatus__subcmd__help__subcmd__rgb__subcmd__timeout"
                ;;
            alatus__subcmd__help__subcmd__rgb,wake)
                cmd="alatus__subcmd__help__subcmd__rgb__subcmd__wake"
                ;;
            alatus__subcmd__help__subcmd__rgb__subcmd__timeout,delay)
                cmd="alatus__subcmd__help__subcmd__rgb__subcmd__timeout__subcmd__delay"
                ;;
            alatus__subcmd__help__subcmd__rgb__subcmd__timeout,policy)
                cmd="alatus__subcmd__help__subcmd__rgb__subcmd__timeout__subcmd__policy"
                ;;
            alatus__subcmd__help__subcmd__session,pixel-refresh)
                cmd="alatus__subcmd__help__subcmd__session__subcmd__pixel__subcmd__refresh"
                ;;
            alatus__subcmd__help__subcmd__session,set)
                cmd="alatus__subcmd__help__subcmd__session__subcmd__set"
                ;;
            alatus__subcmd__help__subcmd__session,status)
                cmd="alatus__subcmd__help__subcmd__session__subcmd__status"
                ;;
            alatus__subcmd__mode,auto)
                cmd="alatus__subcmd__mode__subcmd__auto"
                ;;
            alatus__subcmd__mode,balanced)
                cmd="alatus__subcmd__mode__subcmd__balanced"
                ;;
            alatus__subcmd__mode,cycle)
                cmd="alatus__subcmd__mode__subcmd__cycle"
                ;;
            alatus__subcmd__mode,full)
                cmd="alatus__subcmd__mode__subcmd__full"
                ;;
            alatus__subcmd__mode,get)
                cmd="alatus__subcmd__mode__subcmd__get"
                ;;
            alatus__subcmd__mode,help)
                cmd="alatus__subcmd__mode__subcmd__help"
                ;;
            alatus__subcmd__mode,performance)
                cmd="alatus__subcmd__mode__subcmd__performance"
                ;;
            alatus__subcmd__mode,quiet)
                cmd="alatus__subcmd__mode__subcmd__quiet"
                ;;
            alatus__subcmd__mode,set)
                cmd="alatus__subcmd__mode__subcmd__set"
                ;;
            alatus__subcmd__mode__subcmd__help,auto)
                cmd="alatus__subcmd__mode__subcmd__help__subcmd__auto"
                ;;
            alatus__subcmd__mode__subcmd__help,balanced)
                cmd="alatus__subcmd__mode__subcmd__help__subcmd__balanced"
                ;;
            alatus__subcmd__mode__subcmd__help,cycle)
                cmd="alatus__subcmd__mode__subcmd__help__subcmd__cycle"
                ;;
            alatus__subcmd__mode__subcmd__help,full)
                cmd="alatus__subcmd__mode__subcmd__help__subcmd__full"
                ;;
            alatus__subcmd__mode__subcmd__help,get)
                cmd="alatus__subcmd__mode__subcmd__help__subcmd__get"
                ;;
            alatus__subcmd__mode__subcmd__help,help)
                cmd="alatus__subcmd__mode__subcmd__help__subcmd__help"
                ;;
            alatus__subcmd__mode__subcmd__help,performance)
                cmd="alatus__subcmd__mode__subcmd__help__subcmd__performance"
                ;;
            alatus__subcmd__mode__subcmd__help,quiet)
                cmd="alatus__subcmd__mode__subcmd__help__subcmd__quiet"
                ;;
            alatus__subcmd__mode__subcmd__help,set)
                cmd="alatus__subcmd__mode__subcmd__help__subcmd__set"
                ;;
            alatus__subcmd__oled,dim)
                cmd="alatus__subcmd__oled__subcmd__dim"
                ;;
            alatus__subcmd__oled,help)
                cmd="alatus__subcmd__oled__subcmd__help"
                ;;
            alatus__subcmd__oled,refresh)
                cmd="alatus__subcmd__oled__subcmd__refresh"
                ;;
            alatus__subcmd__oled,status)
                cmd="alatus__subcmd__oled__subcmd__status"
                ;;
            alatus__subcmd__oled__subcmd__help,dim)
                cmd="alatus__subcmd__oled__subcmd__help__subcmd__dim"
                ;;
            alatus__subcmd__oled__subcmd__help,help)
                cmd="alatus__subcmd__oled__subcmd__help__subcmd__help"
                ;;
            alatus__subcmd__oled__subcmd__help,refresh)
                cmd="alatus__subcmd__oled__subcmd__help__subcmd__refresh"
                ;;
            alatus__subcmd__oled__subcmd__help,status)
                cmd="alatus__subcmd__oled__subcmd__help__subcmd__status"
                ;;
            alatus__subcmd__power,help)
                cmd="alatus__subcmd__power__subcmd__help"
                ;;
            alatus__subcmd__power,ppt)
                cmd="alatus__subcmd__power__subcmd__ppt"
                ;;
            alatus__subcmd__power__subcmd__help,help)
                cmd="alatus__subcmd__power__subcmd__help__subcmd__help"
                ;;
            alatus__subcmd__power__subcmd__help,ppt)
                cmd="alatus__subcmd__power__subcmd__help__subcmd__ppt"
                ;;
            alatus__subcmd__power__subcmd__help__subcmd__ppt,list)
                cmd="alatus__subcmd__power__subcmd__help__subcmd__ppt__subcmd__list"
                ;;
            alatus__subcmd__power__subcmd__help__subcmd__ppt,set)
                cmd="alatus__subcmd__power__subcmd__help__subcmd__ppt__subcmd__set"
                ;;
            alatus__subcmd__power__subcmd__ppt,help)
                cmd="alatus__subcmd__power__subcmd__ppt__subcmd__help"
                ;;
            alatus__subcmd__power__subcmd__ppt,list)
                cmd="alatus__subcmd__power__subcmd__ppt__subcmd__list"
                ;;
            alatus__subcmd__power__subcmd__ppt,set)
                cmd="alatus__subcmd__power__subcmd__ppt__subcmd__set"
                ;;
            alatus__subcmd__power__subcmd__ppt__subcmd__help,help)
                cmd="alatus__subcmd__power__subcmd__ppt__subcmd__help__subcmd__help"
                ;;
            alatus__subcmd__power__subcmd__ppt__subcmd__help,list)
                cmd="alatus__subcmd__power__subcmd__ppt__subcmd__help__subcmd__list"
                ;;
            alatus__subcmd__power__subcmd__ppt__subcmd__help,set)
                cmd="alatus__subcmd__power__subcmd__ppt__subcmd__help__subcmd__set"
                ;;
            alatus__subcmd__rgb,help)
                cmd="alatus__subcmd__rgb__subcmd__help"
                ;;
            alatus__subcmd__rgb,off)
                cmd="alatus__subcmd__rgb__subcmd__off"
                ;;
            alatus__subcmd__rgb,on)
                cmd="alatus__subcmd__rgb__subcmd__on"
                ;;
            alatus__subcmd__rgb,set-brightness)
                cmd="alatus__subcmd__rgb__subcmd__set__subcmd__brightness"
                ;;
            alatus__subcmd__rgb,set-color)
                cmd="alatus__subcmd__rgb__subcmd__set__subcmd__color"
                ;;
            alatus__subcmd__rgb,status)
                cmd="alatus__subcmd__rgb__subcmd__status"
                ;;
            alatus__subcmd__rgb,sync)
                cmd="alatus__subcmd__rgb__subcmd__sync"
                ;;
            alatus__subcmd__rgb,timeout)
                cmd="alatus__subcmd__rgb__subcmd__timeout"
                ;;
            alatus__subcmd__rgb,wake)
                cmd="alatus__subcmd__rgb__subcmd__wake"
                ;;
            alatus__subcmd__rgb__subcmd__help,help)
                cmd="alatus__subcmd__rgb__subcmd__help__subcmd__help"
                ;;
            alatus__subcmd__rgb__subcmd__help,off)
                cmd="alatus__subcmd__rgb__subcmd__help__subcmd__off"
                ;;
            alatus__subcmd__rgb__subcmd__help,on)
                cmd="alatus__subcmd__rgb__subcmd__help__subcmd__on"
                ;;
            alatus__subcmd__rgb__subcmd__help,set-brightness)
                cmd="alatus__subcmd__rgb__subcmd__help__subcmd__set__subcmd__brightness"
                ;;
            alatus__subcmd__rgb__subcmd__help,set-color)
                cmd="alatus__subcmd__rgb__subcmd__help__subcmd__set__subcmd__color"
                ;;
            alatus__subcmd__rgb__subcmd__help,status)
                cmd="alatus__subcmd__rgb__subcmd__help__subcmd__status"
                ;;
            alatus__subcmd__rgb__subcmd__help,sync)
                cmd="alatus__subcmd__rgb__subcmd__help__subcmd__sync"
                ;;
            alatus__subcmd__rgb__subcmd__help,timeout)
                cmd="alatus__subcmd__rgb__subcmd__help__subcmd__timeout"
                ;;
            alatus__subcmd__rgb__subcmd__help,wake)
                cmd="alatus__subcmd__rgb__subcmd__help__subcmd__wake"
                ;;
            alatus__subcmd__rgb__subcmd__help__subcmd__timeout,delay)
                cmd="alatus__subcmd__rgb__subcmd__help__subcmd__timeout__subcmd__delay"
                ;;
            alatus__subcmd__rgb__subcmd__help__subcmd__timeout,policy)
                cmd="alatus__subcmd__rgb__subcmd__help__subcmd__timeout__subcmd__policy"
                ;;
            alatus__subcmd__rgb__subcmd__timeout,delay)
                cmd="alatus__subcmd__rgb__subcmd__timeout__subcmd__delay"
                ;;
            alatus__subcmd__rgb__subcmd__timeout,help)
                cmd="alatus__subcmd__rgb__subcmd__timeout__subcmd__help"
                ;;
            alatus__subcmd__rgb__subcmd__timeout,policy)
                cmd="alatus__subcmd__rgb__subcmd__timeout__subcmd__policy"
                ;;
            alatus__subcmd__rgb__subcmd__timeout__subcmd__help,delay)
                cmd="alatus__subcmd__rgb__subcmd__timeout__subcmd__help__subcmd__delay"
                ;;
            alatus__subcmd__rgb__subcmd__timeout__subcmd__help,help)
                cmd="alatus__subcmd__rgb__subcmd__timeout__subcmd__help__subcmd__help"
                ;;
            alatus__subcmd__rgb__subcmd__timeout__subcmd__help,policy)
                cmd="alatus__subcmd__rgb__subcmd__timeout__subcmd__help__subcmd__policy"
                ;;
            alatus__subcmd__session,help)
                cmd="alatus__subcmd__session__subcmd__help"
                ;;
            alatus__subcmd__session,pixel-refresh)
                cmd="alatus__subcmd__session__subcmd__pixel__subcmd__refresh"
                ;;
            alatus__subcmd__session,set)
                cmd="alatus__subcmd__session__subcmd__set"
                ;;
            alatus__subcmd__session,status)
                cmd="alatus__subcmd__session__subcmd__status"
                ;;
            alatus__subcmd__session__subcmd__help,help)
                cmd="alatus__subcmd__session__subcmd__help__subcmd__help"
                ;;
            alatus__subcmd__session__subcmd__help,pixel-refresh)
                cmd="alatus__subcmd__session__subcmd__help__subcmd__pixel__subcmd__refresh"
                ;;
            alatus__subcmd__session__subcmd__help,set)
                cmd="alatus__subcmd__session__subcmd__help__subcmd__set"
                ;;
            alatus__subcmd__session__subcmd__help,status)
                cmd="alatus__subcmd__session__subcmd__help__subcmd__status"
                ;;
            *)
                ;;
        esac
    done

    case "${cmd}" in
        alatus)
            opts="-h -V --gui --tray --help --version gui status capabilities watch monitor mode thermal profile fan cycle quiet balanced perf performance full oled display power charge-limit battery charge rgb daemon completions session help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__balanced)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__capabilities)
            opts="-j -h --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__charge__subcmd__limit)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__completions)
            opts="-h --help bash elvish fish powershell zsh"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__cycle)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon)
            opts="-h --help session system status help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__help)
            opts="session system status help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__help__subcmd__session)
            opts="start stop restart status run"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__help__subcmd__session__subcmd__restart)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__help__subcmd__session__subcmd__run)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__help__subcmd__session__subcmd__start)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__help__subcmd__session__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__help__subcmd__session__subcmd__stop)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__help__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__help__subcmd__system)
            opts="start stop restart status run"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__help__subcmd__system__subcmd__restart)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__help__subcmd__system__subcmd__run)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__help__subcmd__system__subcmd__start)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__help__subcmd__system__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__help__subcmd__system__subcmd__stop)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__session)
            opts="-h --help start stop restart status run help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__session__subcmd__help)
            opts="start stop restart status run help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__session__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__session__subcmd__help__subcmd__restart)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__session__subcmd__help__subcmd__run)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__session__subcmd__help__subcmd__start)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__session__subcmd__help__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__session__subcmd__help__subcmd__stop)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__session__subcmd__restart)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__session__subcmd__run)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__session__subcmd__start)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__session__subcmd__status)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__session__subcmd__stop)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__status)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__system)
            opts="-h --help start stop restart status run help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__system__subcmd__help)
            opts="start stop restart status run help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__system__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__system__subcmd__help__subcmd__restart)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__system__subcmd__help__subcmd__run)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__system__subcmd__help__subcmd__start)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__system__subcmd__help__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__system__subcmd__help__subcmd__stop)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__system__subcmd__restart)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__system__subcmd__run)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__system__subcmd__start)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__system__subcmd__status)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__daemon__subcmd__system__subcmd__stop)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__display)
            opts="-h --help status rate dim mux overdrive od help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__display__subcmd__dim)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__display__subcmd__help)
            opts="status rate dim mux overdrive help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__display__subcmd__help__subcmd__dim)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__display__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__display__subcmd__help__subcmd__mux)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__display__subcmd__help__subcmd__overdrive)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__display__subcmd__help__subcmd__rate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__display__subcmd__help__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__display__subcmd__mux)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__display__subcmd__overdrive)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__display__subcmd__rate)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__display__subcmd__status)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan)
            opts="-h --help get set balanced quiet performance full cycle help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan__subcmd__balanced)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan__subcmd__cycle)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan__subcmd__full)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan__subcmd__get)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan__subcmd__help)
            opts="get set balanced quiet performance full cycle help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan__subcmd__help__subcmd__balanced)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan__subcmd__help__subcmd__cycle)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan__subcmd__help__subcmd__full)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan__subcmd__help__subcmd__get)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan__subcmd__help__subcmd__performance)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan__subcmd__help__subcmd__quiet)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan__subcmd__help__subcmd__set)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan__subcmd__performance)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan__subcmd__quiet)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__fan__subcmd__set)
            opts="-h --help balanced quiet performance full"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__full)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__gui)
            opts="-m -h --tray --minimized --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help)
            opts="gui status capabilities watch monitor mode fan cycle quiet balanced perf full oled display power charge-limit rgb daemon completions session help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__balanced)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__capabilities)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__charge__subcmd__limit)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__completions)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__cycle)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__daemon)
            opts="session system status"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__daemon__subcmd__session)
            opts="start stop restart status run"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__daemon__subcmd__session__subcmd__restart)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__daemon__subcmd__session__subcmd__run)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__daemon__subcmd__session__subcmd__start)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__daemon__subcmd__session__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__daemon__subcmd__session__subcmd__stop)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__daemon__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__daemon__subcmd__system)
            opts="start stop restart status run"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__daemon__subcmd__system__subcmd__restart)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__daemon__subcmd__system__subcmd__run)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__daemon__subcmd__system__subcmd__start)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__daemon__subcmd__system__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__daemon__subcmd__system__subcmd__stop)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__display)
            opts="status rate dim mux overdrive"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__display__subcmd__dim)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__display__subcmd__mux)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__display__subcmd__overdrive)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__display__subcmd__rate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__display__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__fan)
            opts="get set balanced quiet performance full cycle"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__fan__subcmd__balanced)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__fan__subcmd__cycle)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__fan__subcmd__full)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__fan__subcmd__get)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__fan__subcmd__performance)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__fan__subcmd__quiet)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__fan__subcmd__set)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__full)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__gui)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__mode)
            opts="get set balanced quiet performance full cycle auto"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__mode__subcmd__auto)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__mode__subcmd__balanced)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__mode__subcmd__cycle)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__mode__subcmd__full)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__mode__subcmd__get)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__mode__subcmd__performance)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__mode__subcmd__quiet)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__mode__subcmd__set)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__monitor)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__oled)
            opts="status dim refresh"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__oled__subcmd__dim)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__oled__subcmd__refresh)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__oled__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__perf)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__power)
            opts="ppt"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__power__subcmd__ppt)
            opts="list set"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__power__subcmd__ppt__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__power__subcmd__ppt__subcmd__set)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__quiet)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__rgb)
            opts="status set-color set-brightness off on wake sync timeout"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__rgb__subcmd__off)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__rgb__subcmd__on)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__rgb__subcmd__set__subcmd__brightness)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__rgb__subcmd__set__subcmd__color)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__rgb__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__rgb__subcmd__sync)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__rgb__subcmd__timeout)
            opts="policy delay"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__rgb__subcmd__timeout__subcmd__delay)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__rgb__subcmd__timeout__subcmd__policy)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__rgb__subcmd__wake)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__session)
            opts="status set pixel-refresh"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__session__subcmd__pixel__subcmd__refresh)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__session__subcmd__set)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__session__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__help__subcmd__watch)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode)
            opts="-h --help get set balanced quiet performance full cycle auto help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__auto)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__balanced)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__cycle)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__full)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__get)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__help)
            opts="get set balanced quiet performance full cycle auto help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__help__subcmd__auto)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__help__subcmd__balanced)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__help__subcmd__cycle)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__help__subcmd__full)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__help__subcmd__get)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__help__subcmd__performance)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__help__subcmd__quiet)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__help__subcmd__set)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__performance)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__quiet)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__mode__subcmd__set)
            opts="-h --help balanced quiet performance full"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__monitor)
            opts="-i -h --interval-ms --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --interval-ms)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -i)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__oled)
            opts="-h --help status dim refresh help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__oled__subcmd__dim)
            opts="-h --dim --level --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --dim)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --level)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__oled__subcmd__help)
            opts="status dim refresh help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__oled__subcmd__help__subcmd__dim)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__oled__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__oled__subcmd__help__subcmd__refresh)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__oled__subcmd__help__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__oled__subcmd__refresh)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__oled__subcmd__status)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__perf)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__power)
            opts="-h --help ppt help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__power__subcmd__help)
            opts="ppt help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__power__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__power__subcmd__help__subcmd__ppt)
            opts="list set"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__power__subcmd__help__subcmd__ppt__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__power__subcmd__help__subcmd__ppt__subcmd__set)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__power__subcmd__ppt)
            opts="-h --list --help list set help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__power__subcmd__ppt__subcmd__help)
            opts="list set help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__power__subcmd__ppt__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__power__subcmd__ppt__subcmd__help__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__power__subcmd__ppt__subcmd__help__subcmd__set)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__power__subcmd__ppt__subcmd__list)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__power__subcmd__ppt__subcmd__set)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__quiet)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb)
            opts="-h --help status set-color set-brightness off on wake sync timeout help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__help)
            opts="status set-color set-brightness off on wake sync timeout help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__help__subcmd__off)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__help__subcmd__on)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__help__subcmd__set__subcmd__brightness)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__help__subcmd__set__subcmd__color)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__help__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__help__subcmd__sync)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__help__subcmd__timeout)
            opts="policy delay"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__help__subcmd__timeout__subcmd__delay)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__help__subcmd__timeout__subcmd__policy)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__help__subcmd__wake)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__off)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__on)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__set__subcmd__brightness)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__set__subcmd__color)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__status)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__sync)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__timeout)
            opts="-p -d -h --policy --delay --help policy delay help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --policy)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --delay)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -d)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__timeout__subcmd__delay)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__timeout__subcmd__help)
            opts="policy delay help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__timeout__subcmd__help__subcmd__delay)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__timeout__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__timeout__subcmd__help__subcmd__policy)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__timeout__subcmd__policy)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__rgb__subcmd__wake)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__session)
            opts="-h --sync-accent --no-accent --auto-refresh --no-refresh --oled-care --no-oled-care --help status set pixel-refresh help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__session__subcmd__help)
            opts="status set pixel-refresh help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__session__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__session__subcmd__help__subcmd__pixel__subcmd__refresh)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__session__subcmd__help__subcmd__set)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__session__subcmd__help__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__session__subcmd__pixel__subcmd__refresh)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__session__subcmd__set)
            opts="-h --accent --refresh --oled-care --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --accent)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --refresh)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --oled-care)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__session__subcmd__status)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__status)
            opts="-j -w -h --json --watch --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        alatus__subcmd__watch)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _alatus -o nosort -o bashdefault -o default alatus
else
    complete -F _alatus -o bashdefault -o default alatus
fi
