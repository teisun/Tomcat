package com.tomcat.controller;

import cn.hutool.core.util.StrUtil;
import com.tomcat.controller.requeset.AuthenticationReq;
import com.tomcat.controller.response.AuthenticationResp;
import com.tomcat.controller.response.UserResp;
import com.tomcat.service.UserService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.context.annotation.Lazy;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.List;


@RestController
@RequestMapping("/auth")
public class AuthController {

  @Lazy
  @Autowired
  private UserService userService;


  @PostMapping("/registerOrLogin")
  public ResponseEntity<AuthenticationResp> registerOrLogin(@RequestBody AuthenticationReq request) {
    AuthenticationResp response = userService.registerOrLogin(request);
    if(response != null){
      return ResponseEntity.ok(response);
    }else {
      return ResponseEntity.badRequest().build();
    }
  }

  @GetMapping("/findByDeviceId")
  public ResponseEntity<List<UserResp>> findByDeviceId(@RequestParam String deviceId) {
    // TODO
    if(StrUtil.isBlank(deviceId)){
      ResponseEntity.badRequest().build();
    }
    List<UserResp> response = userService.findByDeviceId(deviceId);
    if(response != null){
      return ResponseEntity.ok(response);
    }else {
      return ResponseEntity.badRequest().build();
    }
  }




}