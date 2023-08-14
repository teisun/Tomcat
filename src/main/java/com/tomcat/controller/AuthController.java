package com.tomcat.controller;

import cn.hutool.core.util.StrUtil;
import com.tomcat.controller.requeset.AuthenticationRequest;
import com.tomcat.controller.response.AuthenticationResponse;
import com.tomcat.controller.response.UserDTO;
import com.tomcat.domain.User;
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
  public ResponseEntity<AuthenticationResponse> registerOrLogin(@RequestBody AuthenticationRequest request) {
    AuthenticationResponse response = userService.registerOrLogin(request);
    if(response != null){
      return ResponseEntity.ok(response);
    }else {
      return ResponseEntity.badRequest().build();
    }
  }

  @GetMapping("/findByDeviceId")
  public ResponseEntity<List<UserDTO>> findByDeviceId(@RequestParam String deviceId) {
    // TODO
    if(StrUtil.isBlank(deviceId)){
      ResponseEntity.badRequest().build();
    }
    List<UserDTO> response = userService.findByDeviceId(deviceId);
    if(response != null){
      return ResponseEntity.ok(response);
    }else {
      return ResponseEntity.badRequest().build();
    }
  }




}