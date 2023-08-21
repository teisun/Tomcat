package com.tomcat.controller;

import cn.hutool.core.util.RandomUtil;
import org.springframework.http.ResponseEntity;
import org.springframework.stereotype.Controller;
import org.springframework.ui.Model;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.servlet.ModelAndView;

@Controller
@RequestMapping("/test")
public class TestWSController {
    @GetMapping("/websocket")
    public ModelAndView index() {
        ModelAndView mav = new ModelAndView("socket");
        mav.addObject("uid", RandomUtil.randomNumbers(6));
        return mav;
    }
    @GetMapping("/websocket1")
    public ResponseEntity<String> test() {
        return ResponseEntity.ok("websocket1 test!!!!! ");
    }
}
