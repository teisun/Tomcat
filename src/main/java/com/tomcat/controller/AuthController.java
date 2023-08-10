package com.tomcat.controller;

import cn.hutool.core.util.StrUtil;
import com.tomcat.controller.request.AuthenticationRequest;
import com.tomcat.controller.response.AuthenticationResponse;
import com.tomcat.domain.User;
import com.tomcat.service.UserService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.context.annotation.Lazy;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.List;


@RestController
@RequestMapping("/auth")
public class AuthController {

  @Lazy
  @Autowired
  private UserService userService;


  @PostMapping("/registerOrLogin")
  public ResponseEntity<AuthenticationResponse> registerOrLogin(@RequestBody AuthenticationRequest request) {
    AuthenticationResponse response = userService.registerOrLogin(request);
    if(response != null){
      return ResponseEntity.ok(response);
    }else {
      return ResponseEntity.badRequest().build();
    }
  }

  @PostMapping("/findByDeviceId")
  public ResponseEntity<List<User>> findByDeviceId(@RequestBody AuthenticationRequest request) {
    if(request == null || StrUtil.isBlank(request.deviceId)){
      ResponseEntity.badRequest().build();
    }
    List<User> response = userService.findByDeviceId(request.deviceId);
    if(response != null){
      return ResponseEntity.ok(response);
    }else {
      return ResponseEntity.badRequest().build();
    }
  }




}